use std::env;

#[derive(Default)]
pub struct Cli {
    pub command: Command,
}

impl std::fmt::Display for Cli {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Usage: chsc <command>")?;
        write!(f, "{}", self.command)
    }
}

impl Cli {

    pub fn usage(&self) {
        print!("{}", self)
    }
}

#[derive(Default)]
pub enum Command {
    #[default]
    Help,
    Compile {
        input_path: String,
    },
    CompileRun {
        input_path: String,
    },
    Version,
}

impl Command {
    fn command_info() -> &'static [CommandInfo] {
        &[
            CommandInfo {
                name: "compile",
                alias: Some("c"),
                args: "<file>",
                description: "Compile the program",
            },
            CommandInfo {
                name: "compile-run",
                alias: Some("cr"),
                args: "<file>",
                description: "Compile and run the program",
            },
            CommandInfo {
                name: "version",
                alias: None,
                args: "",
                description: "Show chsc version",
            },
            CommandInfo {
                name: "help",
                alias: None,
                args: "",
                description: "Show help information",
            },
        ]
    }
}

impl std::fmt::Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Commands:")?;

        let command_info = Command::command_info();
        let pad = {
            command_info.iter().fold(0, |acc, ci| {
                acc.max(ci.name.len() + ci.alias.map(|a| a.len()).unwrap_or(0) + ci.args.len())
            })
        };
        for cmd in command_info {
            if cmd.args.is_empty() {
                writeln!(
                    f,
                    "   {}{sep}{desc}",
                    cmd.name,
                    sep = " ".repeat(pad - cmd.name.len() + 1),
                    desc = cmd.description
                )?;
            } else {
                writeln!(
                    f,
                    "   {} {}{sep}{desc}",
                    cmd.name,
                    cmd.args,
                    sep = " ".repeat(pad - (cmd.name.len() + cmd.args.len())),
                    desc = cmd.description
                )?;
            }
        }

        writeln!(f, "Aliases:")?;

        for cmd in command_info {
            if let Some(alias) = cmd.alias {
                writeln!(
                    f,
                    "   {alias}{sep}Alias For {name}",
                    name = cmd.name,
                    sep = " ".repeat(pad - alias.len() + 1)
                )?;
            }
        }

        Ok(())
    }
}

struct CommandInfo {
    name: &'static str,
    alias: Option<&'static str>,
    args: &'static str,
    description: &'static str,
}

impl Cli {
    pub fn parse() -> Result<Self, ()> {
        let args: Vec<String> = env::args().skip(1).collect();

        if args.is_empty() {
            return Ok(Self::default());
        }

        let command_name = &args[0];

        let command = match command_name.as_str() {
            "version" => Command::Version,
            "help" | "-h" | "--help" => Command::Help,

            "compile" | "c" => match args.get(1) {
                Some(path) => Command::Compile {
                    input_path: path.clone(),
                },
                None => {
                    eprintln!("Error: 'compile' requires an input file");
                    return Err(());
                }
            },

            "compile-run" | "cr" => match args.get(1) {
                Some(path) => Command::CompileRun {
                    input_path: path.clone(),
                },
                None => {
                    eprintln!("Error: 'compile-run' requires an input file");
                    return Err(());
                }
            },

            _ => {
                eprintln!("Error: Unknown command '{}'", command_name);
                eprintln!("Use 'chsc help' to see available commands");
                return Err(());
            }
        };

        Ok(Cli { command })
    }
}

// To add a new command, you need to:
// 1. Add a variant to the Command enum
// 2. Add a case in the match statement in parse() or parse_extended()
// 3. Add command info to CommandInfo array
//
// Example of adding a new "run" command:
/*
In Command enum:
    Run { input_path: String },

In command_info():
    CommandInfo {
        name: "run",
        args: "<input-file.chs>",
        description: "Run a compiled program",
    },

In parse_extended():
    "run" | "r" => {
        match args.get(1) {
            Some(path) => Command::Run { input_path: path.clone() },
            None => {
                eprintln!("Error: 'run' requires an input file");
                return None;
            }
        }
    }
*/
