use std::time::Duration;

use futures_timer::ext::TryFutureExt;
use serde::Deserialize;
use clap::{Arg, App};

type MyResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

#[derive(Deserialize, Debug)]
struct AdvisorApp {
    name: String,
    location: String,
}

impl AdvisorApp {
    fn healthcheck(&self) -> String {
        format!("{}/healthcheck", self.location)
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

async fn get(endpoint: String) -> MyResult<String> {
    let mut res = surf::get(endpoint).timeout(Duration::from_secs(1)).await?;
    res.body_string().await
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
                          .get_matches();

    let app_name = matches.value_of("app_name").unwrap();

    let mut settings = config::Config::default();
    settings.merge(config::File::with_name(".advisor"));

    let config = settings.try_into::<Config>().unwrap();

    let app = config.for_app(app_name).unwrap();

    let value = get(app.healthcheck()).await?;
    println!("{}", value);
    Ok(())
}
