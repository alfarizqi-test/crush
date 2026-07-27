// Test wrapper interpreter logic manual
// Run: cargo test wrapper_tests

#[cfg(test)]
mod wrapper_tests {
    use std::collections::HashMap;
    use crate::config::wrapper::expand_vars;

    #[test]
    fn test_expand_vars_empty_cwd() {
        let mut locals = HashMap::new();
        locals.insert("cwd".into(), "".into());
        locals.insert("PWD".into(), "/home/user".into());

        let result = expand_vars("\"$cwd\"", &locals, &[]);
        assert_eq!(result, "\"\""); // quotes preserved, cwd empty inside
    }

    #[test]
    fn test_expand_vars_with_path() {
        let mut locals = HashMap::new();
        locals.insert("cwd".into(), "/tmp/testdir".into());
        locals.insert("tmp".into(), "/tmp/yazi-abc".into());

        let result = expand_vars("cd \"$cwd\"", &locals, &[]);
        assert_eq!(result, "cd \"/tmp/testdir\"");
    }

    #[test]
    fn test_expand_args() {
        let mut locals = HashMap::new();
        locals.insert("args".into(), "file.txt dir/".into());

        let result = expand_vars("yazi $args --cwd-file=\"/tmp/x\"", &locals, &[]);
        assert_eq!(result, "yazi file.txt dir/ --cwd-file=\"/tmp/x\"");
    }
}
