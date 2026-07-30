use crate::*;

pub(crate) fn git_bytes(repository: &Path, args: &[String]) -> Result<Vec<u8>, String> {
    let executable = find_executable(&["git"]).ok_or_else(|| "Git is not installed.".to_owned())?;
    let safe = format!("safe.directory={}", repository.to_string_lossy());
    let mut command = StdCommand::new(executable);
    hide_std_command_window(&mut command);
    let output = command
        .arg("-c")
        .arg(safe)
        .args(args)
        .current_dir(repository)
        .output()
        .map_err(|error| format!("Failed to run Git: {error}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let command = format!("git {}", args.join(" "));
        let detail = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            "Git returned no diagnostic output.".to_owned()
        };
        Err(format!(
            "{command} failed with {}.\n{detail}",
            output.status
        ))
    }
}

pub(crate) fn git(repository: &Path, args: &[String]) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&git_bytes(repository, args)?)
        .trim()
        .to_owned())
}

pub(crate) fn git_static(repository: &Path, args: &[&str]) -> Result<String, String> {
    git(
        repository,
        &args
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>(),
    )
}

pub(crate) fn repository_has_head(repository: &Path) -> Result<bool, String> {
    match git_static(repository, &["rev-parse", "--verify", "HEAD"]) {
        Ok(_) => Ok(true),
        Err(error) if is_unborn_head_error(&error) => Ok(false),
        Err(error) => Err(error),
    }
}

fn is_unborn_head_error(error: &str) -> bool {
    let normalized = error.to_ascii_lowercase();
    normalized.contains("ambiguous argument 'head'")
        || normalized.contains("unknown revision")
        || normalized.contains("needed a single revision")
}

pub(crate) fn git_status(repository: &Path) -> String {
    git_static(repository, &["status", "--short", "--branch"])
        .unwrap_or_else(|error| format!("Git evidence unavailable: {error}"))
}

pub(crate) fn review_mutation_guard(repository: &Path) -> Result<String, String> {
    let head = git_static(repository, &["rev-parse", "HEAD"])?;
    let status = git_static(repository, &["status", "--porcelain"])?;
    let mut hasher = Sha256::new();
    hasher.update(status.as_bytes());
    Ok(format!("{head}:{:x}", hasher.finalize()))
}

pub(crate) fn changed_files(repository: &Path, base_head: &str) -> Vec<String> {
    let mut files = BTreeSet::new();
    if let Ok(output) = git_static(repository, &["status", "--short"]) {
        for line in output.lines() {
            if let Some(path) = line.get(3..) {
                if !path.trim().is_empty() {
                    files.insert(path.trim().to_owned());
                }
            }
        }
    }
    if let Ok(output) = git_static(repository, &["diff", "--name-only", base_head, "HEAD"]) {
        files.extend(
            output
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(str::to_owned),
        );
    }
    files.into_iter().collect()
}

pub(crate) fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    const MARKER: &str = "\n[truncated by Agent Room]";
    if max_bytes <= MARKER.len() {
        let mut end = max_bytes;
        while end > 0 && !value.is_char_boundary(end) {
            end -= 1;
        }
        return value[..end].to_owned();
    }
    let mut end = max_bytes - MARKER.len();
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{}", &value[..end], MARKER)
}

pub(crate) fn append_capped(output: &mut String, line: &str) {
    if output.len() >= PROCESS_OUTPUT_LIMIT {
        return;
    }
    let remaining = PROCESS_OUTPUT_LIMIT - output.len();
    let value = format!("{line}\n");
    output.push_str(&truncate_utf8(&value, remaining));
}

pub(crate) fn read_context_file(path: &Path, max_bytes: usize) -> String {
    std::fs::read_to_string(path)
        .map(|value| truncate_utf8(&value, max_bytes))
        .unwrap_or_default()
}

pub(crate) fn relative_path(repository: &Path, path: &Path) -> String {
    path.strip_prefix(repository)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(crate) fn collect_named_files(
    repository: &Path,
    directory: &Path,
    names: &[&str],
    depth: usize,
    maximum: usize,
    files: &mut Vec<PathBuf>,
) {
    if depth > 8 || files.len() >= maximum {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        if files.len() >= maximum {
            break;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_file() && names.iter().any(|candidate| *candidate == name) {
            files.push(path);
        } else if file_type.is_dir()
            && !matches!(
                name.as_str(),
                ".git" | "node_modules" | "target" | "dist" | ".vite" | "artifacts"
            )
            && path.starts_with(repository)
        {
            collect_named_files(repository, &path, names, depth + 1, maximum, files);
        }
    }
}

pub(crate) fn discover_instruction_files(repository: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_named_files(
        repository,
        repository,
        &["AGENTS.md", "CLAUDE.md"],
        0,
        80,
        &mut files,
    );
    files.sort_by_key(|path| relative_path(repository, path));
    files
}

pub(crate) fn instruction_applies(
    instruction: &Path,
    repository: &Path,
    changed_files: &[String],
) -> bool {
    let directory = instruction.parent().unwrap_or(repository);
    if directory == repository {
        return true;
    }
    let prefix = relative_path(repository, directory);
    changed_files.iter().any(|changed| {
        let normalized = changed.replace('\\', "/");
        normalized == prefix || normalized.starts_with(&format!("{prefix}/"))
    })
}

pub(crate) fn repository_instructions(
    repository: &Path,
    changed_files: &[String],
) -> (String, Vec<String>) {
    let discovered = discover_instruction_files(repository);
    let paths = discovered
        .iter()
        .map(|path| relative_path(repository, path))
        .collect::<Vec<_>>();
    let mut sections = discovered
        .iter()
        .filter(|path| instruction_applies(path, repository, changed_files))
        .map(|path| {
            let relative = relative_path(repository, path);
            format!(
                "## {relative}\n{}",
                read_context_file(path, SOURCE_BUDGET_BYTES)
            )
        })
        .collect::<Vec<_>>();
    let nested = discovered
        .iter()
        .filter(|path| !instruction_applies(path, repository, changed_files))
        .map(|path| format!("- {}", relative_path(repository, path)))
        .collect::<Vec<_>>();
    if !nested.is_empty() {
        sections.push(format!(
            "## Nested instruction inventory\nOpen and follow a nested instruction file before editing files in its directory scope:\n{}",
            nested.join("\n")
        ));
    }
    (sections.join("\n\n"), paths)
}

pub(crate) fn parse_skill_metadata(path: &Path) -> (String, String) {
    let content = read_context_file(path, 4 * 1024);
    let mut name = path
        .parent()
        .and_then(Path::file_name)
        .and_then(OsStr::to_str)
        .unwrap_or("project-skill")
        .to_owned();
    let mut description = String::new();
    if content.starts_with("---") {
        for line in content.lines().skip(1) {
            let trimmed = line.trim();
            if trimmed == "---" {
                break;
            }
            if let Some(value) = trimmed.strip_prefix("name:") {
                name = value.trim().trim_matches(['"', '\'']).to_owned();
            } else if let Some(value) = trimmed.strip_prefix("description:") {
                description = value.trim().trim_matches(['"', '\'']).to_owned();
            }
        }
    }
    (name, description)
}

pub(crate) fn objective_terms(objective: &str) -> HashSet<String> {
    const STOP_WORDS: &[&str] = &[
        "and", "for", "from", "into", "the", "this", "that", "then", "use", "with",
    ];
    objective
        .split(|character: char| !character.is_alphanumeric() && character != '-')
        .map(str::to_ascii_lowercase)
        .filter(|term| term.len() >= 3 && !STOP_WORDS.contains(&term.as_str()))
        .collect()
}

pub(crate) fn select_project_skills(repository: &Path, objective: &str) -> Vec<SelectedSkill> {
    let terms = objective_terms(objective);
    let objective_lower = objective.to_ascii_lowercase();
    let mut candidates = Vec::new();
    for root in [
        ".codex/skills",
        ".agents/skills",
        ".claude/skills",
        "skills",
    ] {
        let path = repository.join(root);
        if !path.is_dir() {
            continue;
        }
        let mut skill_files = Vec::new();
        collect_named_files(repository, &path, &["SKILL.md"], 0, 80, &mut skill_files);
        for skill_path in skill_files {
            let (name, description) = parse_skill_metadata(&skill_path);
            let haystack = format!("{} {}", name, description).to_ascii_lowercase();
            let mut score = terms
                .iter()
                .filter(|term| haystack.contains(term.as_str()))
                .count();
            if objective_lower.contains(&format!("${}", name.to_ascii_lowercase()))
                || objective_lower.contains(&name.to_ascii_lowercase())
            {
                score += 8;
            }
            if score > 0 {
                candidates.push(SelectedSkill {
                    relative_path: relative_path(repository, &skill_path),
                    name,
                    description,
                    score,
                });
            }
        }
    }
    candidates.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    candidates.truncate(MAX_SELECTED_SKILLS);
    candidates
}

pub(crate) fn skill_context(skills: &[SelectedSkill]) -> String {
    if skills.is_empty() {
        return String::new();
    }
    let entries = skills
        .iter()
        .map(|skill| {
            format!(
                "- `{}`: {}{}",
                skill.relative_path,
                skill.name,
                if skill.description.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", skill.description)
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Agent Room selected these repository-local skills for this objective:\n{entries}\n\nBefore taking action, open every selected SKILL.md from the managed worktree and read it completely. Follow referenced resources only when the skill routes the current task to them. Provider-global skills remain provider-owned and are not copied into the context packet."
    )
}

pub(crate) fn project_memory(repository: &Path) -> String {
    read_context_file(
        &repository.join(".agent-room").join("memory.md"),
        SOURCE_BUDGET_BYTES / 2,
    )
}
