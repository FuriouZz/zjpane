use shlex::Shlex;

pub struct UserCommand {
    pub args: Vec<String>,
}

impl UserCommand {
    pub fn set_command(&mut self, command: &str) {
        let lex = Shlex::new(command);
        self.args = lex.collect();
    }
}
