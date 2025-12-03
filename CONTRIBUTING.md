# 🤝 Contributing to VES

Thank you for your interest in contributing :)
VES is still in its early stage (`v0.1.0`), so contributions of **any size** — from filing issues, adding features to just using VES and giving your thoughts — are valuable. Before you start writing any code you should:

- Be willing to sign the contributor's agreement. This is triggered on your first PR contribution, signing the CLA allows us to review and accept your amazing contribution PRs.

---

## 📌 How to Contribute

### 1. Report Issues
- Use the **Bug Report** issue template if you find a bug.
- Use the **Feature Request** issue template for new ideas/features.
- Use the **Roadmap Module** issue template to help track roadmap items, or create issues for any additions to the current `v1.0.0` roadmap track.

### 2. Suggest Improvements
- Discussions are open — share ideas, ask questions, or give feedback.
- PRs are welcome for almost anything at this stage; **documentation improvements**, **feature requests**, **and just about anything relevant**.

### 3. Code Contributions
- Fork the repo, clone it locally and go to the root directory:
```bash
git clone https://github.com/H3IMD3LL-Labs-Inc/VES.git
cd VES
```
- Create a branch in the root directory of your local VES repo based on your contribution using the following syntax:
```bash
git checkout -b [feature, improvement or other contribution]/what-the-branch-is-for
```
- Write clear, understandable commit messages, for example:
```bash
git commit -m "what-the-feature-does-feature"
```
- Ensure your code is formatted, tested and complies with our code style.
- Submit a PR referencing the related issue you have worked on and await review and approval.

VES will try to respond to PRs in a timely manner to ensure contributors can get back in the flow and not lose interest in their individual contributions.

---

## ✅ Development Environment Setup (for VES Core)

1. Setup your development environment
- Install Rust (latest stable release version):
  - Linux, macOS, Unix-like OS:
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
  - After installation, run the following command to ensure everything is setup correctly:
  ```bash
  rustc --version
  ```

  - For **Windows OS**, go to the [Rust official website](rust-lang.org/tools/install/) and download the installer for your PC architecture `32-bit` or `64-bit`:

  ![Rust windows OS installer website](/assets/images/rust_windows_installer_page.jpeg)

- Install Protoc Pre-compiled Binaries (Any OS)
  - To install the latest release of the protocol compiler from pre-compiled binaries, follow these [instructions](https://protobuf.dev/installation/)

2. Clone the repo locally:
   ```bash
   git clone https://github.com/H3IMD3LL-Labs-Inc/VES.git
   cd VES.git
   ```

3. Run tests in the project directory on your machine:
   ```bash
   cargo test
   ```

## 🔒 Security

Please do not report security vulnerabilities in public issues or PRs. See our [Security Policy](./SECURITY.md) for responsible disclosure guidelines.

## 📍 Roadmap

You can view VES's roadmap in [README.md#roadmap](./README.md#roadmap).

To help with a roadmap item:
- Open a Roadmap Module issue.
- Break the work into subtasks, if possible to plan and track progress.
- Submit PRs linked to that issue.

## 🙏 Community
- Be respectful and constructive in discussions.
- Ask questions — there are no bad questions, but there can be bad answers.
- Early contributors will help shape the direction of VES.

## 💡 Questions?

- Open/Join a [GitHub Discussion](https://github.com/H3IMD3LL-Labs-Inc/VES/discussions)
- Email any other [questions](mailto:dennis.njuguna@heimdelllabs.cloud)
