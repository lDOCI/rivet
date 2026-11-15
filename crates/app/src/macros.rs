//! The macro list, each with its star, and the favourites themselves.

use slint::{Model, ModelRc, SharedString, VecModel};

use crate::{AppWindow, MacroRow};

/// Every macro in the list's order, and whether it is a favourite. A favourite
/// the printer no longer has is not shown.
pub fn rows(all: &[String], favorites: &[String]) -> Vec<(String, bool)> {
    all.iter()
        .map(|m| (m.clone(), favorites.contains(m)))
        .collect()
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
    let all = strings(&ui.get_macros());
    let favorites = strings(&ui.get_favorite_macros());
    let rows: Vec<MacroRow> = rows(&all, &favorites)
        .into_iter()
        .map(|(name, fav)| MacroRow {
            name: name.into(),
            fav,
        })
        .collect();
    let starred: Vec<MacroRow> = rows.iter().filter(|r| r.fav).cloned().collect();
    ui.set_favorite_macro_rows(ModelRc::new(VecModel::from(starred)));
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
    fn each_macro_with_its_star() {
        let all = s(&["A", "B", "C"]);
        assert_eq!(
            rows(&all, &s(&["C", "GONE"])),
            vec![("A".into(), false), ("B".into(), false), ("C".into(), true)]
        );
    }

    #[test]
    fn a_star_toggles() {
        let f = toggled(&[], "A");
        assert_eq!(f, s(&["A"]));
        assert_eq!(toggled(&f, "A"), Vec::<String>::new());
    }
}
