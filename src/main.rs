use std::time::Duration;

use futures_timer::ext::TryFutureExt;
use serde::Deserialize;
use clap::{Arg, App, SubCommand, ArgMatches};
use snafu::Snafu;
use prettytable::{Table, row, cell};

type MyResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;
type SnafuResult<T> = Result<T, Error>;


#[derive(Snafu, Debug)]
enum Error {
    #[snafu(display("Could not open config"))]
    CouldNotFindConfig{},
    #[snafu(display("Error reading remote API"))]
    RemoteAPIError{},
    #[snafu(display("Error reading remote API"))]
    CommandNotFound,
}

#[derive(Eq, PartialEq, Debug)]
enum Command {
    Healthcheck,
    ShowQuestionnaires,
    ShowPeople,
    DeletePerson{email: String},
    CreatePerson(PersonParams),
    AddPersonToQuestionnaire{id: String, email: String},
    RemovePersonFromQuestionnaire{id: String, email: String},
    Unexpected,
}

enum Authentication {
    None,
    Token(String)
}

#[derive(Deserialize, Debug)]
struct AdvisorApp {
    name: String,
    location: String,
    token: String,
}

struct StringWriter {
    content: String
}

impl StringWriter {
    fn new() -> Self {
        StringWriter { content: String::new() }
    }
}

impl std::io::Write for StringWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let addition = std::str::from_utf8(&buf).unwrap();

        self.content.push_str(addition);

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}


impl AdvisorApp {
    async fn run(&self, command: Command) -> SnafuResult<String> {
        use Command::*;

        match command {
            Healthcheck => get(self.healthcheck(), Authentication::None).await,
            ShowPeople => self.show_people().await,
                _ => Err(Error::CommandNotFound),
        }
    }

    fn healthcheck(&self) -> String {
        format!("{}/healthcheck", self.location)
    }

    async fn show_people(&self) -> SnafuResult<String> {
        let url = format!("{}/admin/people", self.location);

        let raw = get(url, Authentication::Token(self.token.clone())).await.expect("read from API");

        let people: People = serde_json::from_str(&raw).expect("Was not able to read API response as JSON");

        let mut table = Table::new();
        table.add_row(row!["Name", "Email", "Is mentor"]);

        for person in people {
            table.add_row(row![person.name, person.email, person.is_mentor]);
        }
        let mut output = StringWriter::new();
        table.print(&mut output).expect("Was not able to write to buffer");

        Result::Ok(output.content)
    }
}


#[derive(Deserialize, Debug)]
struct Person {
    name: String,
    email: String,
    is_mentor: bool,
}

type People = Vec<Person>;

#[derive(Deserialize, Debug)]
struct Config {
    default: Option<String>,
    apps: Vec<AdvisorApp>
}

impl Config {
    fn for_app(&self, name: &str) -> Option<&AdvisorApp> {
        self.apps.iter().find(|a| a.name == name)
    }
}

async fn get(endpoint: String, auth: Authentication) -> SnafuResult<String> {
    let mut req = surf::get(endpoint);

    if let Authentication::Token(token) = auth {
        req = req.set_header("Authorization", format!("Bearer {}", token));
    }

    let mut res = req.timeout(Duration::from_secs(5)).await.or_else(|_| RemoteAPIError.fail() )?;

    res.body_string().await.or_else(|_| RemoteAPIError.fail())
}

fn load_config() -> SnafuResult<Config> {
    let mut settings = config::Config::default();
    settings.merge(config::File::with_name(".advisor"));

    settings.try_into::<Config>().or_else(|_| CouldNotFindConfig.fail())
}

type PersonParams = std::collections::HashMap<String, String>;

fn string(m: &ArgMatches, name: &'static str) -> String {
    m.value_of(name).expect(&format!("'{}' is marked as required", name)).to_owned()
}

impl Command {
    fn get() -> (Option<String>, Command) {
        let email = Arg::with_name("email").takes_value(true).required(true).validator(has_at);

        let matches = App::new("Advisor-CLI")
            .version("0.1")
            .author("Felipe Sere felipe@sere.dev>")
            .about("Managing instances of advisor")
            .arg(Arg::with_name("app_name")
                .short("a")
                .long("app")
                .value_name("APP")
                .help("Which app to act upon. Overrides default in .advisor.json")
                .takes_value(true))
            .subcommand(SubCommand::with_name("show")
                .arg(Arg::with_name("kind").takes_value(true).required(true).possible_values(&["people", "questionnaires"]))
            )
            .subcommand(SubCommand::with_name("delete").arg(&email))
            .subcommand(SubCommand::with_name("update")
                .arg(Arg::with_name("questionnaire_id").takes_value(true).required(true))
                .arg(Arg::with_name("mode").takes_value(true).required(true).possible_values(&["add", "remove"]))
                .arg(&email)
            )
            .subcommand(SubCommand::with_name("health"))
            .get_matches();

        let app_name = matches.value_of("app_name").map(|val| val.to_owned());

        (app_name, Command::parse(&matches))
    }

    fn parse(matches: &ArgMatches) -> Command {
        use Command::*;

        if let Some(_) = matches.subcommand_matches("health") {
            return Healthcheck;
        }

        if let Some(m) = matches.subcommand_matches("show") {
            match m.value_of("kind") {
                Some("people") => return ShowPeople,
                Some("questionnaires") => return ShowQuestionnaires,
                None | Some(_) => unreachable!("'kind' is marked as required only allowed to be one of two values"),
            }
        }

        if let Some(m) = matches.subcommand_matches("delete") {
            let email = string(m, "email");
            return DeletePerson{ email }
        }

        if let Some(m) = matches.subcommand_matches("update") {
            let id = string(m, "questionnaire_id");
            let email = string(m, "email");

            match m.value_of("mode") {
                Some("add") =>  return AddPersonToQuestionnaire{id , email},
                Some("remove") =>  return RemovePersonFromQuestionnaire{id, email},
                None | Some(_) => unreachable!("'mode' is marked as required and one of two values")
            }
        }

        Unexpected
    }
}
fn has_at(v: String) -> Result<(), String> {
    if v.contains("@") { return Ok(()); }
    Err(String::from("The value did not contain the required @ sigil"))
}



#[runtime::main]
async fn main() -> MyResult<()> {
    let (app_name, c) = Command::get();

    println!("Comand: {:?}", c);

    let config = load_config().expect("was not able to find a config");

    let name = app_name.or(config.default.clone()).expect("need to specific which app to use");

    let app = config.for_app(&name).expect(&format!("unable to find app {}", name));

    match app.run(c).await {
        Ok(value) => println!("Success: {}\n", value),
        Err(e) => println!("Failure: {}", e),
    }

    Ok(())
}
