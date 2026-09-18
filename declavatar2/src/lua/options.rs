use mlua::{Error as LuaError, FromLua, Result as LuaResult, Table, Value};

/// Reader for an options table that rejects any key its builder does not know.
pub struct Options {
    owner: &'static str,
    table: Option<Table>,
    known: Vec<&'static str>,
}

impl Options {
    pub fn new(owner: &'static str, table: Option<Table>) -> Self {
        Self {
            owner,
            table,
            known: Vec::new(),
        }
    }

    pub fn take<T: FromLua>(&mut self, key: &'static str) -> LuaResult<Option<T>> {
        self.known.push(key);
        let Some(table) = &self.table else {
            return Ok(None);
        };
        table
            .get::<Option<T>>(key)
            .map_err(|error| LuaError::runtime(format!("{}: option `{key}`: {error}", self.owner)))
    }

    pub fn finish(self) -> LuaResult<()> {
        let Some(table) = &self.table else {
            return Ok(());
        };

        let mut unknown = Vec::new();
        for pair in table.pairs::<Value, Value>() {
            let (key, _) = pair?;
            match key.as_string() {
                Some(name) if self.known.contains(&name.to_string_lossy().as_str()) => (),
                Some(name) => unknown.push(format!("`{}`", name.to_string_lossy())),
                None => unknown.push(format!("<{}>", key.type_name())),
            }
        }
        if unknown.is_empty() {
            return Ok(());
        }

        unknown.sort();
        Err(LuaError::runtime(format!(
            "{}: unknown option {} (known options are {})",
            self.owner,
            unknown.join(", "),
            self.known.join(", "),
        )))
    }
}

#[cfg(test)]
mod tests {
    use mlua::Lua;
    use rstest::*;

    use super::*;

    fn table(lua: &Lua, source: &str) -> Table {
        lua.load(source).eval().expect("table should be built")
    }

    #[rstest]
    fn known_options_are_taken_and_missing_ones_are_none() {
        let lua = Lua::new();
        let mut options = Options::new("da.int", Some(table(&lua, r#"{ default = 3 }"#)));

        assert_eq!(options.take::<i64>("default").unwrap(), Some(3));
        assert_eq!(options.take::<String>("scope").unwrap(), None);
        assert!(options.finish().is_ok());
    }

    #[rstest]
    fn an_absent_table_yields_no_options() {
        let mut options = Options::new("da.int", None);

        assert_eq!(options.take::<i64>("default").unwrap(), None);
        assert!(options.finish().is_ok());
    }

    #[rstest]
    fn a_leftover_key_is_rejected_with_the_known_ones() {
        let lua = Lua::new();
        let mut options = Options::new("da.group_layer", Some(table(&lua, r#"{ driven_by = "Emote", symetric = true }"#)));

        options.take::<String>("driven_by").unwrap();
        options.take::<bool>("symmetric").unwrap();
        let error = options.finish().expect_err("the typo should be rejected");
        let message = error.to_string();

        assert!(message.contains("da.group_layer: unknown option `symetric`"), "{message}");
        assert!(message.contains("known options are driven_by, symmetric"), "{message}");
    }

    #[rstest]
    fn every_leftover_key_is_listed_at_once() {
        let lua = Lua::new();
        let mut options = Options::new("da.avatar", Some(table(&lua, r#"{ menus = {}, exported = {}, [1] = "x" }"#)));

        options.take::<Table>("menu").unwrap();
        let error = options.finish().expect_err("the typos should be rejected");
        let message = error.to_string();

        assert!(message.contains("`exported`"), "{message}");
        assert!(message.contains("`menus`"), "{message}");
        assert!(message.contains("<integer>"), "{message}");
    }

    #[rstest]
    fn a_value_of_the_wrong_type_names_the_option() {
        let lua = Lua::new();
        let mut options = Options::new("da.int", Some(table(&lua, r#"{ default = "three" }"#)));

        let error = options.take::<i64>("default").expect_err("the value should be rejected");
        let message = error.to_string();

        assert!(message.contains("da.int: option `default`"), "{message}");
    }
}
