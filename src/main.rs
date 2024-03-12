use dotenv::dotenv;

use curl::easy::{Easy, List};
use serde_json;

use std::fs::OpenOptions;
use std::io::Write;
use std::str::FromStr;

use dialoguer::{theme::ColorfulTheme, Select};

fn download_gitignore(html_url: &str) {
    let raw_url = html_url.replace("/blob/", "/raw/");

    let mut gitignore_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(".gitignore")
        .expect("error opening file");

    let mut easy = Easy::new();
    easy.url(&raw_url).unwrap();
    let _ = easy.follow_location(true);
    let mut gitignore_data = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer
            .write_function(|data| {
                gitignore_data.extend_from_slice(data);
                Ok(data.len())
            })
            .unwrap();
        transfer.perform().unwrap();
    }
    let _ = gitignore_file.write(&gitignore_data);
}

fn pick_and_download_gitignore(language: &str, github_token: &str) {
    // init easy handle and url
    let mut easy = Easy::new();
    easy.url(
        &format!("https://api.github.com/search/code?q=repo%3Agithub%2Fgitignore+{language}")[..],
    )
    .unwrap();

    // init required headers for github api interfacing
    let mut headers = List::new();
    headers
        .append("Accept: application/vnd.github+json")
        .expect("accept header incorrect");
    headers
        .append(&format!("Authorization: Bearer {github_token}")[..])
        .expect("auth token incorrect");
    headers
        .append("User-Agent: gitignore-add")
        .expect("user agent incorrect");
    easy.http_headers(headers).expect("wrong headers");

    // write http response to buffer, and convert to json
    let mut search_response = Vec::new();
    {
        let mut transfer = easy.transfer();
        transfer
            .write_function(|data| {
                search_response.extend_from_slice(data);
                Ok(data.len())
            })
            .unwrap();
        transfer.perform().unwrap();
    }
    let search_response = String::from_utf8(search_response).unwrap();
    let search_response = serde_json::Value::from_str(&search_response).unwrap();

    // download .gitignore directly or show a prompt incase of multiple matches
    if search_response["total_count"] == 1 {
        let html_url = search_response["items"][0]["html_url"].as_str().unwrap();
        download_gitignore(&html_url);
    } else {
        let mut options = Vec::new();
        let mut urls = Vec::new();

        for item in search_response["items"].as_array().unwrap() {
            options.push(item["name"].as_str().unwrap());
            urls.push(item["html_url"].as_str().unwrap());
        }

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Which .gitignore do you want to use?")
            .items(&options)
            .interact()
            .unwrap();

        download_gitignore(urls[selection]);
    }
}

fn main() {
    // env variables
    dotenv().ok();
    let github_token = std::env::var("GITHUB_TOKEN").expect("GitHub Token must be set");

    // retrieve cli arguments
    let args: Vec<String> = std::env::args().collect();
    let language = &args[1]; // language

    pick_and_download_gitignore(language, &github_token);
}
