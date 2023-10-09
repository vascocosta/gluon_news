#![allow(non_snake_case)]

use chrono::{DateTime, Utc};
use dioxus::prelude::*;
use dioxus_desktop::{Config, WindowBuilder};
use feed_rs::{
    model::{Entry, Feed},
    parser,
};
use futures::future::join_all;
use lazy_static::lazy_static;
use reqwest::{Client, Response};
use serde::Deserialize;
use std::{error::Error, fs::read_to_string, sync::Arc};

lazy_static! {
    static ref SETTINGS: Arc<Settings> = Arc::new(read_settings().unwrap_or_default());
}

#[derive(Deserialize)]
struct Settings {
    feeds: Vec<String>,
    maximized: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            feeds: vec!["https://github.com/vascocosta/gluon_news/commits.atom".to_owned()],
            maximized: true,
        }
    }
}

#[derive(PartialEq, Props)]
struct EntryProps {
    title: String,
    summary: String,
    link: String,
    category: String,
    published: DateTime<Utc>,
}

fn read_settings() -> Result<Settings, Box<dyn Error>> {
    let data = read_to_string("settings.json")?;
    let settings: Settings = serde_json::from_str(&data)?;

    Ok(settings)
}

async fn fetch_news(urls: &[&str]) -> Option<Vec<(String, Entry)>> {
    let client = Client::new();
    let mut tasks = Vec::new();
    for url in urls {
        tasks.push(tokio::task::spawn(
            client.get(*url).header("User-Agent", "gluon_news").send(),
        ));
    }
    let mut outputs = Vec::new();
    for task in tasks {
        outputs.push(task.await.unwrap());
    }
    let responses: Vec<Response> = outputs.into_iter().filter_map(|r| r.ok()).collect();
    let texts: Vec<String> = join_all(responses.into_iter().map(|r| r.text()))
        .await
        .into_iter()
        .filter_map(|r| r.ok())
        .collect();
    let feeds: Vec<Feed> = texts
        .into_iter()
        .filter_map(|t| parser::parse(t.as_bytes()).ok())
        .collect();
    let mut entries: Vec<(String, Entry)> = Vec::new();

    for feed in feeds {
        for entry in feed.entries {
            let feed_title = match feed.title.clone() {
                Some(feed_title) => feed_title.content,
                None => String::from("N/A"),
            };
            entries.push((feed_title, entry));
        }
    }

    if entries.is_empty() {
        return None;
    }

    entries.sort_by(|a, b| {
        b.1.published
            .unwrap_or_default()
            .cmp(&a.1.published.unwrap_or_default())
    });

    Some(entries)
}

fn Entry(cx: Scope<EntryProps>) -> Element {
    cx.render(rsx! {
        div {
            a {
                href: "{cx.props.link}",
                target: "_blank",
                "{cx.props.title}",
            }
        }
        hr {}
        div {
            class: "summary",
            dangerous_inner_html: "{cx.props.summary}",
        }
        hr {}
        div {
            "{cx.props.category}",
        }
        div {
            "{cx.props.published}",
        }
    })
}

fn App(cx: Scope) -> Element {
    let mut count = use_state(cx, || 0);
    let future = use_future(cx, (count,), |_| async move {
        let feeds: Vec<&str> = SETTINGS.feeds.iter().map(|f| f.as_str()).collect();

        fetch_news(&feeds).await
    });

    cx.render(match future.value() {
        Some(response) => rsx! {
            style { include_str!("../style.css") }

            match response {
                Some(feeds) => rsx! {
                    ul {
                        li {
                            button {onclick: move |_| {count += 1}, "Refresh"}
                        }
                        for e in feeds {
                            li {
                                Entry {
                                    title: match e.1.title.clone() {
                                        Some(title) => title.content,
                                        None => String::from("N/A"),
                                    },
                                    summary: match e.1.summary.clone() {
                                        Some(summary) => summary.content.replace("href", ""),
                                        None => String::from("N/A"),
                                    },
                                    link: match e.1.links.get(0) {
                                        Some(link) => link.href.clone(),
                                        None => String::from("N/A"),
                                    },
                                    category: e.0.chars().take(100).collect::<String>(),
                                    published: e.1.published.unwrap_or_default(),
                                }
                            }
                        }
                    }
                },
                None => rsx! {
                    ul {
                        li {
                            div {"Could not fetch any news. Make sure you have a valid settings.json file."}
                        }
                    }
                },
            }
        },
        None => rsx! { div { "Loading..." } },
    })
}

#[tokio::main]
async fn main() {
    dioxus_desktop::launch_cfg(
        App,
        Config::default().with_window(
            WindowBuilder::new()
                .with_title("Gluon News")
                .with_maximized(SETTINGS.maximized)
                .with_resizable(true)
                .with_inner_size(dioxus_desktop::wry::application::dpi::LogicalSize::new(
                    1000.0, 800.0,
                )),
        ),
    );
}
