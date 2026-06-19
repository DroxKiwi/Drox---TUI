//! Skills locaux — catalogue `.drox/skills/<name>/SKILL.md` (sprint §2.31).

mod catalog;

pub use catalog::{
    find_skill, format_skills_listing_for_prompt, load_skills_catalog, model_visible_skills,
    read_skill_file, SkillEntry, SkillError, MAX_SKILL_BYTES_RETURNED, SKILLS_SUBDIR,
};
