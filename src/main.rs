use std::time::Duration;

use futures_timer::ext::TryFutureExt;
use serde::Deserialize;
use clap::{Arg, App, SubCommand};
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
    Unknown(Vec<String>),
}

impl Command {
    fn parse(arguments: Vec<String>) -> Command {
        use Command::*;

        if arguments == vec!("show".to_string(), "people".to_string()) {
            return ShowPeople;
        }

        if arguments == vec!("show".to_string(), "questionnares".to_string()) {
            return ShowQuestionnaires;
        }

        if arguments.get(0) == Some(&"delete".to_string()) {
            if let Some(email) = arguments.get(1) {
                return DeletePerson { email: email.clone() }
            }
        }

        let add = "add".to_string();
        let remove = "remove".to_string();

        if arguments.get(0) == Some(&"update".to_string()) {
            match arguments.get(2) {
                Some(a) if a == &add => return AddPersonToQuestionnaire { id: arguments[1].clone(), email: arguments[3].clone() },
                Some(a) if a == &remove => return RemovePersonFromQuestionnaire { id: arguments[1].clone(), email: arguments[3].clone() },
                None | Some(_) => (),
            }
        }

        if arguments.get(0) == Some(&"create".to_string()) && arguments.get(1) == Some(&"person".to_string()) {
            let remainder = arguments.iter().skip(2).collect::<Vec<_>>();
            for chunk in remainder.chunks_exact(2) {
            }
        }

        Unknown(arguments)
    }
}
fn has_at(v: String) -> Result<(), String> {
    if v.contains("@") { return Ok(()); }
    Err(String::from("The value did not contain the required @ sigil"))
}


#[runtime::main]
async fn main() -> MyResult<()> {
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
        .subcommand(SubCommand::with_name("delete")
            .arg(Arg::with_name("email").takes_value(true).required(true).validator(has_at))
        )
        .subcommand(SubCommand::with_name("update")
            .arg(Arg::with_name("questionnaire_id").takes_value(true).required(true))
            .arg(Arg::with_name("mode").takes_value(true).required(true).possible_values(&["add", "remove"]))
            .arg(Arg::with_name("email").takes_value(true).required(true).validator(has_at))
        )
        .get_matches();

    if let Some(m) = matches.subcommand_matches("show") {
        println!("Running the show command with {:?}", m.value_of("kind"))
    }

    if let Some(m) = matches.subcommand_matches("delete") {
        println!("Running the delete command with {:?}", m.value_of("email"))
    }

    if let Some(m) = matches.subcommand_matches("update") {
        println!("Running the update command to {:?} with questionnaire {:?} and email {:?}",m.value_of("mode"), m.value_of("questionnaire_id"), m.value_of("email"))
    }

    let config = load_config().expect("was not able to find a config");

    let app_name = matches.value_of("app_name").unwrap();

    let app = config.for_app(app_name).expect(&format!("unable to find app {}", app_name));


    /*
    match get(app.healthcheck()).await {
        Ok(value) => println!("Success: {}", value),
        Err(e) => println!("Failure: {}", e),
    }
    */

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    macro_rules! strings {
        ($($x:expr),*) => (vec![$($x.to_string()),*]);
    }


    #[test]
    fn test_parse_unknown() {
        let command = Command::parse(strings!("foo", "bar"));

        assert_eq!(command, Command::Unknown(strings!("foo", "bar")))
    }

    #[test]
    fn test_parse_show_people() {
        let command = Command::parse(strings!("show", "people"));

        assert_eq!(command, Command::ShowPeople)
    }

    #[test]
    fn test_parse_show_questionnaires() {
        let command = Command::parse(strings!("show", "questionnares"));

        assert_eq!(command, Command::ShowQuestionnaires)
    }

    #[test]
    fn test_parse_delete_person() {
        let command = Command::parse(strings!("delete", "a@b.com"));

        assert_eq!(command, Command::DeletePerson{email: "a@b.com".to_string()})
    }

    #[test]
    fn test_add_person_to_questionnaire() {
        let command = Command::parse(strings!("update", "123a", "add", "a@b.com"));

        assert_eq!(command, Command::AddPersonToQuestionnaire{id: "123a".to_string(), email: "a@b.com".to_string()})
    }

    #[test]
    fn test_remove_person_from_questionnaire() {
        let command = Command::parse(strings!("update", "123a", "remove", "a@b.com"));

        assert_eq!(command, Command::RemovePersonFromQuestionnaire{id: "123a".to_string(), email: "a@b.com".to_string()})
    }

    #[test]
    fn test_create_person() {
        let command = Command::parse(strings!("create", "person", "--email", "a@b.com", "--name", "Steve"));

        let mut params: HashMap<String, String> = HashMap::new();
        params.insert("name".to_string(), "Steve".to_string());
        params.insert("email".to_string(), "a@b.com".to_string());

        assert_eq!(command, Command::CreatePerson(params))
    }
}
