//! The macro list with the favourites first, and the favourites themselves.

use slint::{Model, ModelRc, SharedString, VecModel};

use crate::{AppWindow, MacroRow};

/// Rows for the list: with favourites, a "fav" group of them (in the list's own
/// order) and an "all" group of the rest; without, just the macros. A favourite
/// the printer no longer has is not shown.
pub fn rows(all: &[String], favorites: &[String]) -> Vec<(&'static str, String, bool)> {
    let fav: Vec<&String> = all.iter().filter(|m| favorites.contains(m)).collect();
    if fav.is_empty() {
        return all.iter().map(|m| ("", m.clone(), false)).collect();
    }
    let mut out = vec![("fav", String::new(), false)];
    out.extend(fav.iter().map(|m| ("", (*m).clone(), true)));
    out.push(("all", String::new(), false));
    out.extend(
        all.iter()
            .filter(|m| !favorites.contains(m))
            .map(|m| ("", m.clone(), false)),
    );
    out
}

/// Star or unstar `name`: the new list of favourites.
pub fn toggled(favorites: &[String], name: &str) -> Vec<String> {
    if favorites.iter().any(|f| f == name) {
        favorites.iter().filter(|f| *f != name).cloned().collect()
    } else {
        let mut out = favorites.to_vec();
        out.push(name.to_owned());
        out
    }
}

fn strings(model: &ModelRc<SharedString>) -> Vec<String> {
    model.iter().map(|s| s.to_string()).collect()
}

/// Rebuild the list from the window's macros and favourites.
pub fn show(ui: &AppWindow) {
    let rows: Vec<MacroRow> = rows(
        &strings(&ui.get_macros()),
        &strings(&ui.get_favorite_macros()),
    )
    .into_iter()
    .map(|(section, name, fav)| MacroRow {
        section: section.into(),
        name: name.into(),
        fav,
    })
    .collect();
    ui.set_macro_rows(ModelRc::new(VecModel::from(rows)));
}

/// Set the favourites (from the printer's saved settings, or a star tapped).
pub fn set_favorites(ui: &AppWindow, favorites: &[String]) {
    let list: Vec<SharedString> = favorites.iter().map(|f| f.as_str().into()).collect();
    ui.set_favorite_macros(ModelRc::new(VecModel::from(list)));
    show(ui);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| (*x).to_owned()).collect()
    }

    #[test]
    fn favourites_first_then_the_rest() {
        let all = s(&["A", "B", "C"]);
        assert_eq!(
            rows(&all, &[]),
            vec![
                ("", "A".into(), false),
                ("", "B".into(), false),
                ("", "C".into(), false)
            ]
        );
        let r = rows(&all, &s(&["C", "GONE"]));
        assert_eq!(
            r,
            vec![
                ("fav", String::new(), false),
                ("", "C".into(), true),
                ("all", String::new(), false),
                ("", "A".into(), false),
                ("", "B".into(), false),
            ]
        );
    }

    #[test]
    fn a_star_toggles() {
        let f = toggled(&[], "A");
        assert_eq!(f, s(&["A"]));
        assert_eq!(toggled(&f, "A"), Vec::<String>::new());
    }
}
