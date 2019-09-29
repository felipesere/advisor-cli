use std::time::Duration;

use futures_timer::ext::TryFutureExt;
use serde::Deserialize;
use clap::{Arg, App, SubCommand, ArgMatches};
use snafu::Snafu;

type MyResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;
type SnafuResult<T> = Result<T, Error>;


#[derive(Snafu, Debug)]
enum Error {
    #[snafu(display("Could not open config"))]
    CouldNotFindConfig{},
    #[snafu(display("Error reading remote API"))]
    RemoteAPIError{},
}

#[derive(Deserialize, Debug)]
struct AdvisorApp {
    name: String,
    location: String,
}

impl AdvisorApp {
    fn healthcheck(&self) -> String {
        format!("{}/healthcheck", self.location)
    }

    fn list_people(&self) -> String {
        format!("{}/admin/people", self.location)
    }
}

#[derive(Deserialize, Debug)]
struct Config {
    apps: Vec<AdvisorApp>
}

impl Config {
    fn for_app(&self, name: &str) -> Option<&AdvisorApp> {
        self.apps.iter().find(|a| a.name == name)
    }
}

async fn get(endpoint: String) -> SnafuResult<String> {
    let mut res = surf::get(endpoint).timeout(Duration::from_secs(1)).await.or_else(|_| RemoteAPIError.fail() )?;

    res.body_string().await.or_else(|_| RemoteAPIError.fail())
}

fn load_config() -> SnafuResult<Config> {
    let mut settings = config::Config::default();
    settings.merge(config::File::with_name(".advisor"));

    settings.try_into::<Config>().or_else(|_| CouldNotFindConfig.fail())
}

type PersonParams = std::collections::HashMap<String, String>;


#[derive(Eq, PartialEq, Debug)]
enum Command {
    ShowQuestionnaires,
    ShowPeople,
    DeletePerson{email: String},
    CreatePerson(PersonParams),
    AddPersonToQuestionnaire{id: String, email: String},
    RemovePersonFromQuestionnaire{id: String, email: String},
    Unexpected,
}

fn string(m: &ArgMatches, name: &'static str) -> String {
    m.value_of(name).expect(&format!("'{}' is marked as required", name)).to_owned()
}

impl Command {
    fn parse(matches: &ArgMatches) -> Command {
        use Command::*;

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
    let email = Arg::with_name("email").takes_value(true).required(true).validator(has_at);

    let matches = App::new("Advisor-CLI")
        .version("0.1")
        .author("Felipe Sere felipe@sere.dev>")
        .about("Managing instances of advisor")
        .arg(Arg::with_name("app_name")
            .short("a")
            .long("app")
            .value_name("APP")
            .help("Which app to act upon")
            .required(true)
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
        .get_matches();

    let app_name = matches.value_of("app_name").unwrap();
    let c = Command::parse(&matches);

    println!("Comand: {:?}", c);

    let config = load_config().expect("was not able to find a config");

    let app = config.for_app(app_name).expect(&format!("unable to find app {}", app_name));

    match get(app.healthcheck()).await {
        Ok(value) => println!("Success: {}", value),
        Err(e) => println!("Failure: {}", e),
    }

    Ok(())
}
