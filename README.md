A preprocessor for [mdBook](https://github.com/rust-lang/mdBook), try to parse and render [GitLab-specific references](https://docs.gitlab.com/ee/user/markdown.html#gitlab-specific-references) to links,
just like in the [GitLab Flavored Markdown](https://docs.gitlab.com/ee/user/markdown.html).

This preprocessor is intent to be used in GitLab CI pipeline to generate mdBook to Pages.

## Features

- [x] project
- [x] issue
- [x] merge request
- [ ] specific user
- [ ] specific group
- [ ] milestone
- [ ] specific commit
- [ ] commit range comparison
- [ ] label

## Getting Started

First, install `mdbook-gitlab-link`

```
cargo install --git "https://github.com/belltoy/mdbook-gitlab-link"
```

Then, add the following lines to your `book.toml` file to enable this preprocessor:

```toml
[preprocessor."gitlab-link"]
gitlab-project-namespace = "myteam"
gitlab-project-name = "myproj"
gitlab-server-url = "https://example.com"
```

Now, you can build:

```
mdbook build
```

And check the links in the generated book.

Note that when running build in GitLab CI pipeline, the configs in `book.toml` will be overridden by the CI environment variables.

- `CI_SERVER_URL`: `gitlab-server-url`
- `CI_PROJECT_NAMESPACE`: `gitlab-project-namespace`
- `CI_PROJECT_NAME`: `gitlab-project-name`

## License

Copyright (c) 2022 Zhongqiu Zhao.

See [LICENSE](LICENSE).
