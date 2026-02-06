# Contributing to VES

## Overview
VES is a high performance, highly configurable and easy-to-understand observability data platform. Before you get started contributing make sure you read these docs; [CODE OF CONDUCT](CODE_OF_CONDUCT.md), [CONTRIBUTOR DOCS](CONTRIBUTORS.md), [LICENSE](LICENSE.md), [README](README.md), [SECURITY](SECURITY.md). It is recommended you understand these docs before you start contributing inorder to give yourself and other contributors an easy time.

## Getting Started
If you're new to contributing to VES, ensure your development environment (e.g., laptop/PC) has the following installed;
- Rust v1.89.0+ [Install Rust](https://rust-lang.org/tools/install/)
- Git [Install Git](https://git-scm.com/install/)
- Protobuf [Install Protobuf](https://protobuf.dev/installation/)
- rust-analyzer: Check your specific IDEs support/extensions
- cargo-watch: For developement auto-reloading
- cargo-nextest: Fast test runs

Other recommendations, but not necessary;
Zed IDE [Install Zed](https://zed.dev/download), with the following extensions;
   - TOML v1.0.1 
   - Dockerfile v0.1.0
   - Proto v0.3.1
   - Tombi | TOML Toolkit v0.2.0
   - GitHub Actions v0.0.1
   - MDX v0.3.0
   - wakatime v0.1.10

These are just "other" recommendations not necessities, feel free to skip them if you already have a fully setup Rust development environment you're comfortable working in.

## How To Contribute

### Reporting and Resolving Issues
- First of all, check our [current issues](https://github.com/H3IMD3LL-Labs-Inc/VES/issues) to confirm your issue isn't a duplicate.
- Use a clear/descriptive title, description and suggestion when creating the issue using the provided issue template.
- Add relevant code blocks, OS information, VES version and or related issue(s).

### Contribution Steps
1. **Fork the latest version of the repo** from GitHub and **clone your forked repo locally**
```bash
git clone https://github.com/H3IMD3LL-Labs-Inc/VES.git
```

2. **Ensure you are in the source directory** of your cloned fork
```bash
cd VES
```

3. **Create a feature branch to work on your changes**, following [BRANCHING GUIDELINES](BRANCHING_GUIDELINES.md)
```bash
git checkout -b <contribution_type>/<name-of-work>
```

4. **Work on your changes on the feature branch you have created**, following [CODE GUIDELINES](CODE_GUIDELINES.md)

5. **Commit your changes with clear and descriptive commit messages**, following [CODE GUIDELINES](CODE_GUIDELINES.md)
```bash
git add <contribution-type>/<name-of-work> \n
git commit -m <clear-short-and-descriptive-commit-message>
```

6. **Push your commited work to your fork**
```bash
git push -u origin <contribution-type>/<name-of-work>
```

7. **Open a PR** against VES' actual `integration` branch

8. **Respond to feedback** from the reviewer

See [BRANCHING GUIDELINES](BRANCHING_GUIDELINES.md), to understand clearly how the above approach works in detail.

Also, check [PR GUIDELINES](PR_GUIDELINES.md) to see how you're PRs should be inorder to be most likely accepted.
