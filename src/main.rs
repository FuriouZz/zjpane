use std::{collections::BTreeMap, path::Path};
use zellij_tile::prelude::*;

mod user_command;
use user_command::UserCommand;

#[derive(Debug)]
enum Mode {
    Pane,
    Command,
}

impl Default for Mode {
    fn default() -> Self {
        Self::Pane
    }
}

#[derive(Default)]
struct State {
    active_tab: usize,
    panes: Vec<PaneInfo>,
    position: usize,
    has_permission_granted: bool,
    commands: Vec<UserCommand>,
    mode: Mode,
}

register_plugin!(State);

impl State {
    fn parse_config(&mut self, user_config: &BTreeMap<String, String>) {
        let keys = user_config.keys().filter(|key| key.starts_with("command_"));

        for key in keys {
            let name = key.split("_").nth(1);
            if name.is_none() {
                continue;
            }
            let name = name.unwrap();

            let command = self.commands.iter_mut().find_map(|entry| {
                if name == &entry.name {
                    return Some(entry);
                }
                None
            });

            let command = if let Some(c) = command {
                c
            } else {
                let index = self.commands.len();
                self.commands.push(UserCommand {
                    name: name.to_string(),
                    args: Vec::new(),
                });
                self.commands.get_mut(index).unwrap()
            };

            if key.ends_with("_command") {
                command.set_command(user_config.get(key).unwrap());
            }
        }
    }

    fn handle_event(&mut self, event: &Event) -> bool {
        let mut should_render = false;
        match event {
            Event::TabUpdate(tabs) => {
                tracing::Span::current().record("event_type", "Event::TabUpdate");
                // tracing::debug!(tabs = ?tabs);

                for tab in tabs {
                    if tab.active {
                        self.active_tab = tab.position;
                        break;
                    }
                }
            }
            Event::PaneUpdate(infos) => {
                tracing::Span::current().record("event_type", "Event::PaneUpdate");
                // tracing::debug!(panes = ?panes);

                self.panes.clear();
                self.position = 0;
                for pane in infos.panes.iter() {
                    if *pane.0 != self.active_tab {
                        continue;
                    }
                    for info in pane.1 {
                        if !info.is_plugin {
                            self.panes.push(info.clone());
                        }
                    }
                }

                should_render = true;
            }
            Event::Key(key) => match key {
                Key::Esc => {
                    self.position = 0;
                    hide_self();
                }
                _ => (),
            },
            _ => (),
        }
        should_render
    }

    fn handle_command_event(&mut self, event: &Event) -> bool {
        let mut should_render = false;
        match event {
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
                        if self.position < self.commands.len() - 1 {
                            self.position += 1;
                        }
                        should_render = true;
                    }
                    Key::Left => {
                        self.position = 0;
                        self.mode = Mode::Pane;
                        should_render = true
                    }
                    Key::Char(c) if (*c as u32) == 10 => {
                        if let Some(command) = self.commands.iter().nth(self.position) {
                            self.position = 0;
                            hide_self();

                            let args = command.args.clone();
                            open_command_pane_floating(
                                CommandToRun::new_with_args(
                                    Path::new(&args[0]),
                                    args[1..].to_vec(),
                                ),
                                None,
                            );
                            // open_command_pane_in_place(CommandToRun::new_with_args(
                            //     Path::new(&args[0]),
                            //     args[1..].to_vec(),
                            // ));
                        }
                    }
                    _ => (),
                }
            }
            _ => (),
        }
        should_render
    }

    fn handle_pane_event(&mut self, event: &Event) -> bool {
        let mut should_render = false;
        match event {
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
                    Key::Right => {
                        self.position = 0;
                        self.mode = Mode::Command;
                        should_render = true
                    }
                    Key::Down => {
                        if self.position < self.panes.len() - 1 {
                            self.position += 1;
                        }
                        should_render = true;
                    }
                    Key::Char(c) if (*c as u32) == 10 => {
                        if let Some(pane) = self.panes.get(self.position) {
                            self.position = 0;
                            hide_self();

                            focus_terminal_pane(pane.id, false);
                        }
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
                let pane = self.panes.iter_mut().find(|pane| pane.title.eq(payload));
                if let Some(pane) = pane {
                    focus_terminal_pane(pane.id, false);
                }
            }
            "execute_at" => {
                if let Ok(Some(command)) = payload
                    .parse::<usize>()
                    .map(|index| self.commands.get(index))
                {
                    let args = command.args.clone();
                    open_command_pane_floating(
                        CommandToRun::new_with_args(Path::new(&args[0]), args[1..].to_vec()),
                        None,
                    );
                }
            }
            "execute" => {
                let command = self
                    .commands
                    .iter_mut()
                    .find(|command| command.name.eq(payload));
                if let Some(command) = command {
                    let args = command.args.clone();
                    open_command_pane_floating(
                        CommandToRun::new_with_args(Path::new(&args[0]), args[1..].to_vec()),
                        None,
                    );
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

        self.parse_config(&configuration);

        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::OpenTerminalsOrPlugins,
            PermissionType::RunCommands,
        ]);

        subscribe(&[
            EventType::TabUpdate,
            EventType::PaneUpdate,
            EventType::Key,
            EventType::PermissionRequestResult,
            EventType::RunCommandResult,
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

        let should_render = self.handle_event(&event);
        let should_render2 = match self.mode {
            Mode::Pane => self.handle_pane_event(&event),
            Mode::Command => self.handle_command_event(&event),
        };

        should_render || should_render2
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
        let (pane_mode_selected, command_mode_selected) = match self.mode {
            Mode::Pane => ("*", " "),
            Mode::Command => (" ", "*"),
        };
        println!(
            "[{}] Pane Mode / [{}] Command Mode",
            pane_mode_selected, command_mode_selected
        );

        match self.mode {
            Mode::Pane => {
                for (i, pane) in self.panes.iter().enumerate() {
                    let selected = if i == self.position { "*" } else { " " };
                    println!("{} #{} {}", selected, pane.id, pane.title);
                }
            }
            Mode::Command => {
                for (i, command) in self.commands.iter().enumerate() {
                    let selected = if i == self.position { "*" } else { " " };
                    println!("{} #{} {}", selected, i, command.name);
                }
            }
        }
    }
}
