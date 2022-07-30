use mdbook::preprocess::{Preprocessor, PreprocessorContext};
use mdbook::book::{Book, BookItem};
use mdbook::errors::Result;
use pulldown_cmark::{Event, Options, Parser, Tag};
use regex::Regex;

pub struct GitlabLink {
    re: Regex,
}

type Cfg<'a> = Option<&'a toml::map::Map<String, toml::Value>>;

enum RefType<'a> {
    Project(&'a str),
    Issue {
        namespace: Option<&'a str>,
        project: Option<&'a str>,
        id: &'a str,
    },
    MergeRequest {
        namespace: Option<&'a str>,
        project: Option<&'a str>,
        id: &'a str,
    },
}

impl Default for GitlabLink {
    fn default() -> Self {
        Self::new()
    }
}

impl GitlabLink {
    fn new() -> Self {
        let re = Regex::new(r"(?x)
            (?:                                     # issue or mr group
                (?:
                    (?P<ns>
                        (?:
                            (?:[a-zA-Z0-9-_\.]+)
                            (?:/(?P<subgroup>[a-zA-Z-_\.]+))?
                        )
                    /)?  # optional namespaces
                    (?P<project>[a-zA-Z0-9-_\.]+)   # project
                )?                                  # optional namespace/project
                (?:
                    (?-x:#(?P<issue>\d+))           # issue id #42
                    |                               # or
                    (?:!(?P<merge_request>\d+))     # merge request id !42
                )
            \b)
            |
            (?:
                (?P<project_ref>[a-zA-Z0-9-_\.]+(/[a-zA-Z0-9-_\.]+)?/[a-zA-Z0-9-_\.]+)>   # project ref, group/project>
            )
            ").unwrap();
        Self {
            re
        }
    }

    fn get_server_url<'a>(&self, cfg: Cfg<'a>) -> &'a str {
        option_env!("CI_SERVER_URL").or_else(|| {
            cfg.and_then(|m| {
                m.get("gitlab-server-url").and_then(|s| s.as_str())
            })
        }).unwrap_or("")
    }

    fn get_current_project<'a>(&self, cfg: Cfg<'a>) -> &'a str {
        option_env!("CI_PROJECT_NAME").or_else(|| {
            cfg.and_then(|m| {
                m.get("gitlab-project-name").and_then(|s| s.as_str())
            })
        }).unwrap_or("")
    }

    fn get_current_namespace<'a>(&self, cfg: Cfg<'a>) -> &'a str {
        option_env!("CI_PROJECT_NAMESPACE").or_else(|| {
            cfg.and_then(|m| {
                m.get("gitlab-project-namespace").and_then(|s| s.as_str())
            })
        }).unwrap_or("")
    }

    fn resolve_ref<'a>(&self, ref_link: RefType<'a>, cfg: Cfg<'_>) -> String {
        match ref_link {
            RefType::Project(s) => {
                format!("[{}>]({}/{})", s, self.get_server_url(cfg), s)
            }
            RefType::Issue { namespace, project, id } => {
                let issue = match (namespace, project) {
                    (Some(n), Some(p)) => format!("{}/{}#{}", n, p, id),
                    (None, Some(p)) => format!("{}#{}", p, id),
                    (_, _) => format!("#{}", id)
                };

                format!("[{}]({}/{}/{}/-/issues/{id})",
                    issue,
                    self.get_server_url(cfg),
                    namespace.unwrap_or_else(|| self.get_current_namespace(cfg)),
                    project.unwrap_or_else(|| self.get_current_project(cfg)),
                )
            }
            RefType::MergeRequest { namespace, project, id } => {
                let mr_name = match (namespace, project) {
                    (Some(n), Some(p)) => format!("{n}/{p}!{id}"),
                    (None, Some(p)) => format!("{p}!{id}"),
                    _ => format!("!{id}"),
                };

                format!("[{mr_name}]({}/{}/{}/-/merge_requests/{id})",
                    self.get_server_url(cfg),
                    namespace.unwrap_or_else(|| self.get_current_namespace(cfg)),
                    project.unwrap_or_else(|| self.get_current_project(cfg)),
                )
            }
        }
    }

    fn replace(&self, content: &str, cfg: Cfg<'_>) -> String {
        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_TABLES);
        opts.insert(Options::ENABLE_FOOTNOTES);
        opts.insert(Options::ENABLE_STRIKETHROUGH);
        opts.insert(Options::ENABLE_TASKLISTS);

        let mut refs = vec![];
        let mut in_skip = false;

        let events = Parser::new_ext(content, opts);
        for (e, span) in events.into_offset_iter() {
            match (in_skip, &e) {
                (false,
                    Event::Start(Tag::CodeBlock(_)) |
                    Event::Start(Tag::Heading(_, _, _)) |
                    Event::Start(Tag::Link(_, _, _)) |
                    Event::Start(Tag::Image(_, _, _))
                ) => {
                    in_skip = true;
                    continue;
                }

                (true,
                    Event::End(Tag::CodeBlock(_)) |
                    Event::End(Tag::Heading(_, _, _)) |
                    Event::End(Tag::Link(_, _, _)) |
                    Event::End(Tag::Image(_, _, _))
                ) => {
                    in_skip = false;
                    continue;
                }

                (false, Event::Text(t)) => {
                    for caps in self.re.captures_iter(t) {
                        let matched = caps.get(0).unwrap();
                        log::debug!("capture: ns: {:?}, project: {:?}, issue: {:?}, merge_request: {:?}\n{:?}",
                            caps.name("ns").map(|s| s.as_str()).unwrap_or(""),
                            caps.name("project").map(|s| s.as_str()).unwrap_or(""),
                            caps.name("issue").map(|s| s.as_str()).unwrap_or(""),
                            caps.name("merge_request").map(|s| s.as_str()).unwrap_or(""),
                            matched,
                        );

                        let namespace = caps.name("ns").map(|s| s.as_str());
                        let project = caps.name("project").map(|s| s.as_str());

                        let s = if let Some(m) = caps.name("project_ref") {
                            RefType::Project(m.as_str())
                        } else if let Some(id) = caps.name("issue") {
                            RefType::Issue { namespace, project, id: id.as_str() }
                        } else if let Some(id) = caps.name("merge_request") {
                            RefType::MergeRequest { namespace, project, id: id.as_str() }
                        } else {
                            continue;
                        };

                        let link = self.resolve_ref(s, cfg);

                        refs.push((link, (span.start + matched.start())..(span.start + matched.end())))
                    }
                }

                _ => {
                    continue;
                }
            }
        }

        let mut content = content.to_string();
        for (link, span) in refs.iter().rev() {
            let pre_content = &content[0..span.start];
            let post_content = &content[span.end..];
            content = format!("{}{}{}", pre_content, link, post_content);
        }

        content
    }
}

impl Preprocessor for GitlabLink {
    fn name(&self) -> &str {
        "gitlab-link"
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book> {
        let cfg = ctx.config.get_preprocessor(self.name());

        book.for_each_mut(|item: &mut BookItem| {

            if let BookItem::Chapter(ref mut chapter) = *item {
                chapter.content = self.replace(&chapter.content, cfg);
            }
        });

        Ok(book)
    }

    fn supports_renderer(&self, renderer: &str) -> bool {
        renderer == "html"
    }
}
