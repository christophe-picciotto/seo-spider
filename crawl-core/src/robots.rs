//! robots.txt fetching and parsing (Vague 1).
//!
//! Implements a pragmatic subset of the Robots Exclusion Protocol sufficient
//! for crawl gating:
//!   - Parse the file into `User-agent` blocks.
//!   - Keep the directives of the block matching our user-agent, else the `*`
//!     block.
//!   - `Disallow` / `Allow` are stored as `(allow, path_prefix)` pairs and
//!     matched by longest-prefix-wins.
//!
//! Robustness: any network or parse failure yields `Robots::empty()` (crawl
//! everything). No method panics.

/// Parsed robots.txt rules. Each rule is `(allow, path_prefix)` where
/// `allow == true` means an `Allow:` directive and `false` a `Disallow:`.
pub struct Robots {
    pub rules: Vec<(bool, String)>,
}

impl Robots {
    /// An empty rule set (allows everything).
    pub fn empty() -> Self {
        Robots { rules: Vec::new() }
    }

    /// Parse the textual content of a robots.txt for the given user-agent.
    ///
    /// Lines are interpreted case-insensitively for directive names. Comments
    /// (`#`) and blank lines are ignored. Directives are grouped by the
    /// preceding `User-agent` line(s). We keep the rules of the group whose
    /// agent token is a case-insensitive substring of `ua` (most specific
    /// match by token length wins), falling back to the `*` group.
    pub fn parse(content: &str, ua: &str) -> Self {
        let ua_lower = ua.to_ascii_lowercase();

        // Rules collected for the wildcard `*` group and for the best specific
        // group matching our UA. `specific_len` tracks how specific that match
        // is (longer agent token = more specific).
        let mut star_rules: Vec<(bool, String)> = Vec::new();
        let mut specific_rules: Vec<(bool, String)> = Vec::new();
        let mut specific_len: usize = 0;

        // Agents declared for the current group, and whether a directive has
        // already appeared since the last User-agent line (consecutive
        // User-agent lines accumulate into one group; a directive closes it).
        let mut current_agents: Vec<String> = Vec::new();
        let mut current_rules: Vec<(bool, String)> = Vec::new();
        let mut seen_directive = false;

        // Flush the current group into the star/specific buckets.
        fn flush(
            current_agents: &Vec<String>,
            current_rules: &Vec<(bool, String)>,
            ua_lower: &str,
            star_rules: &mut Vec<(bool, String)>,
            specific_rules: &mut Vec<(bool, String)>,
            specific_len: &mut usize,
        ) {
            if current_rules.is_empty() {
                return;
            }
            for agent in current_agents {
                if agent == "*" {
                    star_rules.extend(current_rules.iter().cloned());
                } else if ua_lower.contains(agent.as_str()) && agent.len() > *specific_len {
                    *specific_rules = current_rules.clone();
                    *specific_len = agent.len();
                }
            }
        }

        for raw_line in content.lines() {
            // Strip inline comments and surrounding whitespace.
            let line = match raw_line.find('#') {
                Some(i) => &raw_line[..i],
                None => raw_line,
            };
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Split on the first ':'; ignore malformed lines.
            let (field, value) = match line.split_once(':') {
                Some((f, v)) => (f.trim().to_ascii_lowercase(), v.trim().to_string()),
                None => continue,
            };

            match field.as_str() {
                "user-agent" => {
                    // A new User-agent line after directives starts a new group.
                    if seen_directive {
                        flush(
                            &current_agents,
                            &current_rules,
                            &ua_lower,
                            &mut star_rules,
                            &mut specific_rules,
                            &mut specific_len,
                        );
                        current_agents.clear();
                        current_rules.clear();
                        seen_directive = false;
                    }
                    current_agents.push(value.to_ascii_lowercase());
                }
                "disallow" => {
                    seen_directive = true;
                    current_rules.push((false, value));
                }
                "allow" => {
                    seen_directive = true;
                    current_rules.push((true, value));
                }
                // Sitemap, Crawl-delay, Host, etc. are ignored here.
                _ => {}
            }
        }

        // Flush the trailing group.
        flush(
            &current_agents,
            &current_rules,
            &ua_lower,
            &mut star_rules,
            &mut specific_rules,
            &mut specific_len,
        );

        // A specific group (matching our UA) overrides the wildcard group.
        let rules = if specific_len > 0 { specific_rules } else { star_rules };
        Robots { rules }
    }

    /// Whether `path` is allowed to be crawled.
    ///
    /// Longest-match-wins: among every rule whose prefix matches the start of
    /// `path`, the one with the longest prefix decides. On a tie, `Allow` wins
    /// (per the common interpretation). An empty `Disallow:` value matches
    /// nothing (everything allowed); an empty `Allow:` value is ignored.
    pub fn allowed(&self, path: &str) -> bool {
        // Match against the path component only (a leading '/' is expected;
        // accept paths that already start with '/').
        let path = if path.is_empty() { "/" } else { path };

        let mut best_len: i64 = -1;
        let mut best_allow = true; // default: allowed

        for (allow, prefix) in &self.rules {
            // Empty Disallow == allow all -> contributes nothing.
            // Empty Allow likewise contributes nothing.
            if prefix.is_empty() {
                continue;
            }
            if path.starts_with(prefix.as_str()) {
                let len = prefix.len() as i64;
                if len > best_len || (len == best_len && *allow) {
                    best_len = len;
                    best_allow = *allow;
                }
            }
        }

        if best_len < 0 {
            // No rule matched -> allowed.
            true
        } else {
            best_allow
        }
    }
}

/// Fetch and parse the robots.txt for a seed URL.
///
/// Downloads `{scheme}://{host}[:{port}]/robots.txt`. Any failure (bad URL,
/// network error, non-2xx, body read error) yields `Robots::empty()` so the
/// crawl proceeds without gating.
pub async fn fetch(client: &reqwest::Client, seed_url: &str, ua: &str) -> Robots {
    let parsed = match url::Url::parse(seed_url) {
        Ok(u) => u,
        Err(_) => return Robots::empty(),
    };

    let host = match parsed.host_str() {
        Some(h) => h,
        None => return Robots::empty(),
    };
    let scheme = parsed.scheme();

    let robots_url = match parsed.port() {
        Some(port) => format!("{scheme}://{host}:{port}/robots.txt"),
        None => format!("{scheme}://{host}/robots.txt"),
    };

    let resp = match client.get(&robots_url).send().await {
        Ok(r) => r,
        Err(_) => return Robots::empty(),
    };

    if !resp.status().is_success() {
        // 404 / 5xx / 401 etc.: treat as "no robots.txt" -> allow everything.
        return Robots::empty();
    }

    match resp.text().await {
        Ok(body) => Robots::parse(&body, ua),
        Err(_) => Robots::empty(),
    }
}
