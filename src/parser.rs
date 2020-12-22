use std::collections::HashMap;

struct Parser<'a, State> {
    header: HashMap<&'a str, &'a str>,
    state: State,
}
