use super::*;
use serde_json::json;

fn run(files: Vec<Value>) -> (Vec<Candidate>, Vec<String>) {
    parse_files(&files.into_iter().map(|v| ("f.json".to_string(), v)).collect::<Vec<_>>())
}

#[test]
fn bifrost_list_with_global_login_on_selected_server() {
    let list = json!([
        {"Name": "Bad", "SiteConfigUrl": "x y/SiteConfig.xml"},
        {"Name": " Local ", "SiteConfigUrl": "localhost/SiteConfig.xml", "NewsFeedUrl": null},
        {"Name": "Remote", "SiteConfigUrl": "Play.Example.com:8080/SiteConfig.xml"},
        null
    ]);
    let cfg = json!({"ServerIndex": 2, "EnableAutoLogin": true,
                     "AutoLoginEmailAddress": "a@b.c", "AutoLoginPassword": "pw"});
    let (c, rej) = run(vec![list, cfg]);
    assert_eq!(c.len(), 2);
    assert_eq!(c[0].name, "Local");
    assert!(!c[0].has_password);
    // index 2 in the original list is "Remote", despite the rejected entry before it
    assert_eq!((c[1].host.as_str(), c[1].email.as_str(), c[1].has_password), ("Play.Example.com:8080", "a@b.c", true));
    assert_eq!(rej.len(), 2); // the bad host and the null entry
}

#[test]
fn bifrost_login_ignored_when_auto_login_off() {
    let list = json!([{"Name": "S", "SiteConfigUrl": "h/SiteConfig.xml"}]);
    let cfg = json!({"ServerIndex": 0, "EnableAutoLogin": false,
                     "AutoLoginEmailAddress": "test1@test.com", "AutoLoginPassword": "123"});
    assert!(!run(vec![list, cfg]).0[0].has_password);
}

#[test]
fn overlay_settings_per_server_login() {
    let s = json!({"servers": [
        {"name": "A", "siteConfigUrl": "https://a.example/SiteConfig.xml", "enableAutoLogin": true,
         "autoLoginEmailAddress": "e@x.y", "autoLoginPassword": "p"},
        {"name": "B", "siteConfigUrl": "b.example/SiteConfig.xml", "enableAutoLogin": false,
         "autoLoginEmailAddress": "e@x.y", "autoLoginPassword": "p"}
    ], "launchOptions": {}});
    let (c, _) = run(vec![s]);
    assert!(c[0].has_password && c[0].use_https);
    assert!(!c[1].has_password);
}

#[test]
fn unrecognised_and_hostile_input() {
    let (c, rej) = run(vec![json!({"foo": 1}), json!("str"),
        json!([{"Name": "N", "SiteConfigUrl": "h/Other.xml"}, {"Name": "Q", "SiteConfigUrl": "h\" -flag/SiteConfig.xml"}])]);
    assert!(c.is_empty());
    assert_eq!(rej.len(), 4);
}

#[test]
fn control_chars_in_password_drop_login() {
    let s = json!({"servers": [{"name": "A", "siteConfigUrl": "a/SiteConfig.xml", "enableAutoLogin": true,
        "autoLoginEmailAddress": "e", "autoLoginPassword": "p\nq"}]});
    assert!(!run(vec![s]).0[0].has_password);
}

#[test]
fn duplicates_marked_case_insensitively_and_within_batch() {
    let (mut c, _) = run(vec![json!([
        {"Name": "1", "SiteConfigUrl": "Foo.com/SiteConfig.xml"},
        {"Name": "2", "SiteConfigUrl": "bar.com/SiteConfig.xml"},
        {"Name": "3", "SiteConfigUrl": "BAR.com/SiteConfig.xml"}])]);
    mark_duplicates(&mut c, &[ex("foo.COM", "x@y.z", "other")], MatchBy::HostEmail);
    assert_eq!(c.iter().map(|c| c.duplicate).collect::<Vec<_>>(), [true, false, true]);
}

fn ex(h: &str, e: &str, n: &str) -> (String, String, String) {
    (h.to_string(), e.to_string(), n.to_string())
}

#[test]
fn match_strategies() {
    let s = json!({"servers": [
        {"name": "Main", "siteConfigUrl": "a.example/SiteConfig.xml", "enableAutoLogin": true,
         "autoLoginEmailAddress": "one@x.y", "autoLoginPassword": "p"},
        {"name": "Alt", "siteConfigUrl": "a.example/SiteConfig.xml", "enableAutoLogin": true,
         "autoLoginEmailAddress": "two@x.y", "autoLoginPassword": "p"}]});
    let existing = [ex("a.example", "one@x.y", "alt")];
    let dups = |by| {
        let (mut c, _) = run(vec![s.clone()]);
        mark_duplicates(&mut c, &existing, by);
        c.iter().map(|c| c.duplicate).collect::<Vec<_>>()
    };
    assert_eq!(dups(MatchBy::Host), [true, true]);
    assert_eq!(dups(MatchBy::HostEmail), [true, false]);
    assert_eq!(dups(MatchBy::HostName), [false, true]);
    assert_eq!(dups(MatchBy::Nothing), [false, false]);
}

#[test]
fn several_accounts_on_one_host_all_import() {
    let s = json!({"servers": [
        {"name": "Main", "siteConfigUrl": "a.example/SiteConfig.xml", "enableAutoLogin": true,
         "autoLoginEmailAddress": "one@x.y", "autoLoginPassword": "p"},
        {"name": "Alt", "siteConfigUrl": "a.example/SiteConfig.xml", "enableAutoLogin": true,
         "autoLoginEmailAddress": "TWO@x.y", "autoLoginPassword": "p"},
        {"name": "Alt again", "siteConfigUrl": "a.example/SiteConfig.xml", "enableAutoLogin": true,
         "autoLoginEmailAddress": "two@x.y", "autoLoginPassword": "p"}]});
    let (mut c, _) = run(vec![s]);
    // "one" already exists on that host; "two" does not
    mark_duplicates(&mut c, &[ex("A.example", "ONE@x.y", "x")], MatchBy::HostEmail);
    assert_eq!(c.iter().map(|c| c.duplicate).collect::<Vec<_>>(), [true, false, true]);
}

#[test]
fn bifrost_dir_found_from_deep_exe_in_versioned_subfolder() {
    let root = std::env::temp_dir().join(format!("mhm-bifrost-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let exe_dir = root.join("Marvel Heroes/UnrealEngine3/Binaries/Win64");
    std::fs::create_dir_all(&exe_dir).unwrap();
    for v in ["Bifrost-0.2.0", "Bifrost-0.3.0"] {
        let d = root.join("Marvel Heroes").join(v);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("Bifrost.ServerList.json"), "[]").unwrap();
    }
    let found = find_bifrost_dir(&exe_dir.join("MarvelHeroesOmega.exe")).unwrap();
    assert!(found.ends_with("Bifrost-0.3.0"));
    assert!(find_bifrost_dir(&root.join("nowhere/game.exe")).is_none());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn email_is_compared_and_emailless_existing_matches() {
    let s = json!({"servers": [{"name": "Dummy", "siteConfigUrl": "a.example/SiteConfig.xml", "enableAutoLogin": true,
        "autoLoginEmailAddress": "a", "autoLoginPassword": "p"}]});
    let dup = |existing: &[(String, String, String)]| {
        let (mut c, _) = run(vec![s.clone()]);
        mark_duplicates(&mut c, existing, MatchBy::HostEmail);
        c[0].duplicate
    };
    assert!(!dup(&[ex("a.example", "b@x.y", "n")])); // same host, different email: new
    assert!(dup(&[ex("a.example", "A", "n")]));      // same email
    assert!(dup(&[ex("a.example", "", "n")]));       // earlier import stored no email
}
