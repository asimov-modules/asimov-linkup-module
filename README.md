# ASIMOV Linkup Module

[![License](https://img.shields.io/badge/license-Public%20Domain-blue.svg)](https://unlicense.org)
[![Compatibility](https://img.shields.io/badge/rust-1.85%2B-blue)](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/)
[![Package](https://img.shields.io/crates/v/asimov-linkup-module)](https://crates.io/crates/asimov-linkup-module)

[ASIMOV] module for import of personal LinkedIn data powered by [Linkup](https://linkupapi.com/).

## ✨ Features

- Imports data about persons and companies on LinkedIn.
- Imports your message inbox, including messages.
- Imports your social graph—your LinkedIn connections.

## 🛠️ Prerequisites

- [Rust](https://rust-lang.org) 1.85+ (2024 edition)

## ⬇️ Installation

### Installation with [ASIMOV CLI] (recommended)

```bash
asimov module install linkup -v
```

### Installation from Source Code

```bash
cargo install asimov-linkup-module
```

## ⚙ Configuration

Credentials can be provided either by using [ASIMOV CLI] module configuration:

```bash
asimov module config linkup -v
```

Or by setting environment variables:

```bash
export LINKUP_API_KEY="..."
export LINKEDIN_EMAIL="..."
export LINKEDIN_PASSWORD="..."
```

## 👉 Examples

### Fetching LinkedIn user info

```bash
asimov-linkup-fetcher https://linkedin.com/in/$USER
```

### Fetching LinkedIn company info

```bash
asimov-linkup-fetcher https://linkedin.com/company/$COMPANY
```

### Listing LinkedIn messaging conversations

```bash
asimov-linkup-fetcher https://linkedin.com/messaging
```

### Listing LinkedIn message thread messages

```bash
asimov-linkup-fetcher https://linkedin.com/messaging/thread/$THREAD
```

### Listing LinkedIn connections

```bash
asimov-linkup-fetcher https://linkedin.com/mynetwork/invite-connect/connections
```

## 👨‍💻 Development

```bash
git clone https://github.com/asimov-modules/asimov-linkup-module.git
```

---

[![Share on X](https://img.shields.io/badge/share%20on-x-03A9F4?logo=x)](https://x.com/intent/post?url=https://github.com/asimov-modules/asimov-linkup-module&text=asimov-linkup-module)
[![Share on Reddit](https://img.shields.io/badge/share%20on-reddit-red?logo=reddit)](https://reddit.com/submit?url=https://github.com/asimov-modules/asimov-linkup-module&title=asimov-linkup-module)
[![Share on Hacker News](https://img.shields.io/badge/share%20on-hn-orange?logo=ycombinator)](https://news.ycombinator.com/submitlink?u=https://github.com/asimov-modules/asimov-linkup-module&t=asimov-linkup-module)
[![Share on Facebook](https://img.shields.io/badge/share%20on-fb-1976D2?logo=facebook)](https://www.facebook.com/sharer/sharer.php?u=https://github.com/asimov-modules/asimov-linkup-module)
[![Share on LinkedIn](https://img.shields.io/badge/share%20on-linkedin-3949AB?logo=linkedin)](https://www.linkedin.com/sharing/share-offsite/?url=https://github.com/asimov-modules/asimov-linkup-module)

[ASIMOV]: https://github.com/asimov-platform
[ASIMOV CLI]: https://github.com/asimov-platform/asimov-cli
