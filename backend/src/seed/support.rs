use crate::models::{Credit, ImageSet};

pub(super) fn episode_title(episode_number: i64) -> &'static str {
    match episode_number {
        1 => "The Signal",
        2 => "Glass Houses",
        3 => "Cold Start",
        4 => "Lattice",
        5 => "The Long Night",
        6 => "Reentry",
        7 => "Parallax",
        8 => "Hollow Frame",
        9 => "Meridian",
        _ => "Afterlight",
    }
}

pub(super) fn asset(kind: &str, id: &str) -> String {
    format!("https://cdn.lifestream.local/{kind}/{id}.jpg")
}

pub(super) fn slugify(input: &str) -> String {
    let mut slug = String::with_capacity(input.len());
    let mut previous_dash = false;
    for character in input.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

pub(super) fn images(id: &str) -> String {
    json(&ImageSet {
        poster: asset("poster", id),
        backdrop: asset("backdrop", id),
        thumbnail: asset("thumb", id),
        logo: None,
    })
    .expect("image set serializes")
}

pub(super) fn credit(id: &str, name: &str, role: &str, character: Option<&str>) -> Credit {
    Credit {
        id: id.to_string(),
        name: name.to_string(),
        role: role.to_string(),
        character: character.map(ToString::to_string),
    }
}

pub(super) fn credits(values: &[Credit]) -> Result<String, sqlx::Error> {
    serde_json::to_string(values).map_err(|error| sqlx::Error::Decode(Box::new(error)))
}

pub(super) fn json<T: serde::Serialize>(value: &T) -> Result<String, sqlx::Error> {
    serde_json::to_string(value).map_err(|error| sqlx::Error::Decode(Box::new(error)))
}
