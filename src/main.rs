use std::collections::BTreeMap;
use zellij_tile::prelude::*;

#[derive(Default)]
struct State {
    active_tab: usize,
    panes: Vec<PaneInfo>,
    position: usize,
    has_permission_granted: bool,
    userspace_configuration: BTreeMap<String, String>,
}

register_plugin!(State);

impl State {
    fn handle_event(&mut self, event: Event) -> bool {
        let mut should_render = false;
        match event {
            Event::TabUpdate(tabs) => {
                tracing::Span::current().record("event_type", "Event::TabUpdate");
                tracing::debug!(tabs = ?tabs);

                for tab in tabs {
                    if tab.active {
                        self.active_tab = tab.position;
                        break;
                    }
                }
            }
            Event::PaneUpdate(panes) => {
                tracing::Span::current().record("event_type", "Event::PaneUpdate");
                tracing::debug!(panes = ?panes);

                self.panes.clear();
                self.position = 0;
                for pane in panes.panes {
                    if pane.0 != self.active_tab {
                        continue;
                    }
                    for info in pane.1 {
                        if !info.is_plugin {
                            self.panes.push(info);
                        }
                    }
                }

                should_render = true;
            }
            Event::Key(key) => {
                tracing::Span::current().record("event_type", "Event::Key");
                tracing::debug!(key = ?key);

                match key {
                    Key::Up => {
                        if self.position > 0 {
                            self.position -= 1;
                        }
                        should_render = true;
                    }
                    Key::Down => {
                        if self.position < self.panes.len() - 1 {
                            self.position += 1;
                        }
                        should_render = true;
                    }
                    Key::Char(c) if (c as u32) == 10 => {
                        if let Some(pane) = self.panes.get(self.position) {
                            focus_terminal_pane(pane.id, false);
                            self.position = 0;
                            hide_self();
                        }
                    }
                    Key::Esc => {
                        self.position = 0;
                        hide_self();
                    }
                    _ => (),
                }
            }
            _ => (),
        }
        should_render
    }

    fn parse_pipe(&mut self, input: &str) -> bool {
        let should_render = false;

        let parts = input.split("::").collect::<Vec<&str>>();

        if parts.len() < 3 {
            return false;
        }

        if parts[0] != "zjpane" {
            return false;
        }

        let action = parts[1];
        let payload = parts[2];
        tracing::debug!(action = ?action);
        tracing::debug!(payload = ?payload);

        match action {
            "focus_at" => {
                if let Ok(Some(pane)) = payload.parse::<usize>().map(|index| self.panes.get(index))
                {
                    focus_terminal_pane(pane.id, false);
                }
            }
            "focus" => {
                let mut pane: Option<&PaneInfo> = None;

                for p in self.panes.iter() {
                    if p.title.eq(payload) {
                        pane = Some(p);
                        break;
                    }
                }

                if let Some(pane) = pane {
                    focus_terminal_pane(pane.id, false);
                }
            }
            _ => (),
        }

        should_render
    }
}

#[cfg(feature = "tracing")]
fn init_tracing() {
    use std::{fs::File, sync::Arc};
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    let file = File::create(".zjpane.log");
    let file = match file {
        Ok(file) => file,
        Err(error) => panic!("Error: {:?}", error),
    };
    let debug_log = tracing_subscriber::fmt::layer().with_writer(Arc::new(file));

    tracing_subscriber::registry().with(debug_log).init();

    tracing::info!("tracing initialized");
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        #[cfg(feature = "tracing")]
        init_tracing();

        self.userspace_configuration = configuration;

        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
        ]);

        subscribe(&[
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::Key,
            EventType::PermissionRequestResult,
        ]);
    }

    #[tracing::instrument(skip_all, fields(event_type))]
    fn update(&mut self, event: Event) -> bool {
        if let Event::PermissionRequestResult(status) = event {
            tracing::Span::current().record("event_type", "Event::PermissionRequestResult");
            tracing::debug!(status = ?status);

            match status {
                PermissionStatus::Granted => self.has_permission_granted = true,
                PermissionStatus::Denied => self.has_permission_granted = false,
            }
        }

        if !self.has_permission_granted {
            return false;
        }

        self.handle_event(event)
    }

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        let mut should_render = false;

        match pipe_message.source {
            PipeSource::Cli(_) | PipeSource::Plugin(_) | PipeSource::Keybind => {
                if let Some(payload) = pipe_message.payload {
                    should_render = self.parse_pipe(&payload);
                }
            }
        }

        should_render
    }

    fn render(&mut self, _rows: usize, _cols: usize) {
        for pane in self.panes.iter() {
            let selected = if pane.id == (self.position as u32) {
                "*"
            } else {
                " "
            };
            println!("{} #{} {}", selected, pane.id, pane.title);
        }
    }
}
