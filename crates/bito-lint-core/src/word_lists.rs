//! Curated word lists for writing analysis.
//!
//! Collections of glue words, transition words, vague words, business jargon,
//! clich\u{e9}s, sensory words, hidden verbs, conjunctions, and spelling pairs.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Common glue/function words (the, a, and, or, etc.).
pub static GLUE_WORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "the", "a", "an", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
        "from", "up", "about", "into", "through", "during", "that", "this", "these", "those", "it",
        "its", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had", "do",
        "does", "did", "will", "would", "should", "could", "may", "might", "must", "can", "which",
        "who", "when", "where", "why", "how", "if", "than", "then", "as", "so",
    ]
    .into_iter()
    .collect()
});

/// Transition words that connect ideas between sentences.
pub static TRANSITION_WORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "however",
        "therefore",
        "thus",
        "consequently",
        "nevertheless",
        "moreover",
        "furthermore",
        "additionally",
        "meanwhile",
        "instead",
        "otherwise",
        "similarly",
        "likewise",
        "conversely",
        "nonetheless",
        "hence",
        "accordingly",
        "subsequently",
        "indeed",
        "specifically",
        "particularly",
        "especially",
    ]
    .into_iter()
    .collect()
});

/// Multi-word transition phrases.
pub static TRANSITION_PHRASES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "for example",
        "for instance",
        "in addition",
        "in contrast",
        "on the other hand",
        "as a result",
        "in conclusion",
        "in summary",
        "to summarize",
        "finally",
    ]
    .into_iter()
    .collect()
});

/// Vague or weak words that weaken prose.
pub static VAGUE_WORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "thing",
        "things",
        "stuff",
        "nice",
        "good",
        "bad",
        "great",
        "terrible",
        "amazing",
        "awesome",
        "interesting",
        "very",
        "really",
        "quite",
        "rather",
        "somewhat",
        "pretty",
        "fairly",
    ]
    .into_iter()
    .collect()
});

/// Vague phrases.
pub static VAGUE_PHRASES: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| ["kind of", "sort of", "a bit"].into_iter().collect());

/// Business jargon words.
pub static BUSINESS_JARGON: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "synergy",
        "leverage",
        "paradigm",
        "disrupt",
        "innovative",
        "streamline",
        "optimization",
        "scalable",
        "bandwidth",
        "win-win",
        "game changer",
        "best practice",
        "core competency",
        "value-added",
        "going forward",
        "deep dive",
        "reach out",
    ]
    .into_iter()
    .collect()
});

/// Business jargon phrases.
pub static BUSINESS_JARGON_PHRASES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "circle back",
        "touch base",
        "low-hanging fruit",
        "move the needle",
        "drink the kool-aid",
        "boil the ocean",
        "think outside the box",
        "at the end of the day",
        "take it offline",
        "drill down",
    ]
    .into_iter()
    .collect()
});

/// Common clich\u{e9}s.
pub static CLICHES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "avoid it like the plague",
        "beat around the bush",
        "better late than never",
        "bite the bullet",
        "break the ice",
        "bring to the table",
        "call it a day",
        "cut to the chase",
        "easy as pie",
        "get the ball rolling",
        "hit the nail on the head",
        "in the nick of time",
        "it goes without saying",
        "jump on the bandwagon",
        "keep your eyes peeled",
        "let the cat out of the bag",
        "piece of cake",
        "raining cats and dogs",
        "the best of both worlds",
        "throw in the towel",
        "time flies",
        "under the weather",
        "when pigs fly",
        "whole nine yards",
        "a blessing in disguise",
        "a dime a dozen",
        "actions speak louder than words",
        "add insult to injury",
        "at the drop of a hat",
        "back to square one",
        "barking up the wrong tree",
        "bent out of shape",
        "bite off more than you can chew",
        "break a leg",
        "burning the midnight oil",
        "caught between a rock and a hard place",
        "costs an arm and a leg",
        "cry over spilled milk",
        "curiosity killed the cat",
        "devil's advocate",
        "don't count your chickens",
        "every cloud has a silver lining",
    ]
    .into_iter()
    .collect()
});

/// Sensory words organized by the five senses.
pub static SENSORY_WORDS: LazyLock<HashMap<&'static str, HashSet<&'static str>>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();

        map.insert(
            "sight",
            [
                "see",
                "saw",
                "seen",
                "look",
                "looked",
                "looking",
                "watch",
                "watched",
                "bright",
                "dark",
                "light",
                "shadow",
                "color",
                "colorful",
                "shiny",
                "dull",
                "vivid",
                "brilliant",
                "gleaming",
                "glowing",
                "sparkling",
                "shimmering",
                "transparent",
                "opaque",
                "visible",
                "invisible",
                "appearance",
                "view",
                "glimpse",
                "glance",
                "stare",
                "gaze",
                "observe",
                "notice",
                "spot",
            ]
            .into_iter()
            .collect(),
        );

        map.insert(
            "sound",
            [
                "hear",
                "heard",
                "listen",
                "listened",
                "sound",
                "noise",
                "loud",
                "quiet",
                "silent",
                "whisper",
                "shout",
                "scream",
                "yell",
                "murmur",
                "mumble",
                "echo",
                "ring",
                "buzz",
                "hum",
                "bang",
                "crash",
                "thump",
                "click",
                "rustle",
                "crackle",
                "pop",
                "snap",
                "sizzle",
                "hiss",
                "roar",
                "howl",
                "musical",
                "melodious",
                "harmonious",
                "deafening",
                "piercing",
            ]
            .into_iter()
            .collect(),
        );

        map.insert(
            "touch",
            [
                "feel", "felt", "touch", "touched", "soft", "hard", "smooth", "rough", "texture",
                "cold", "hot", "warm", "cool", "freezing", "burning", "icy", "sticky", "slippery",
                "dry", "wet", "moist", "damp", "sharp", "dull", "coarse", "silky", "velvety",
                "grainy", "bumpy", "prickly", "tender", "firm", "solid", "squishy", "fluffy",
                "crisp", "brittle",
            ]
            .into_iter()
            .collect(),
        );

        map.insert(
            "smell",
            [
                "smell",
                "smelled",
                "smelling",
                "scent",
                "odor",
                "aroma",
                "fragrance",
                "perfume",
                "stink",
                "stench",
                "whiff",
                "sniff",
                "fragrant",
                "aromatic",
                "pungent",
                "acrid",
                "musty",
                "moldy",
                "fresh",
                "stale",
                "rancid",
                "sweet",
                "sour",
                "spicy",
                "floral",
                "earthy",
                "smoky",
                "putrid",
            ]
            .into_iter()
            .collect(),
        );

        map.insert(
            "taste",
            [
                "taste",
                "tasted",
                "tasting",
                "flavor",
                "flavored",
                "sweet",
                "sour",
                "bitter",
                "salty",
                "savory",
                "spicy",
                "tangy",
                "tart",
                "bland",
                "mild",
                "delicious",
                "tasty",
                "appetizing",
                "mouthwatering",
                "scrumptious",
                "palatable",
                "flavorful",
                "zesty",
                "peppery",
                "sugary",
                "acidic",
            ]
            .into_iter()
            .collect(),
        );

        map
    });

/// Hidden verbs: noun forms that could be replaced with their verb equivalents.
pub static HIDDEN_VERBS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    [
        ("decision", "decide"),
        ("conclusion", "conclude"),
        ("assumption", "assume"),
        ("observation", "observe"),
        ("consideration", "consider"),
        ("implementation", "implement"),
        ("investigation", "investigate"),
        ("examination", "examine"),
        ("explanation", "explain"),
        ("discussion", "discuss"),
        ("analysis", "analyze"),
        ("recommendation", "recommend"),
        ("suggestion", "suggest"),
        ("description", "describe"),
    ]
    .into_iter()
    .collect()
});

/// Coordinating conjunctions.
pub static CONJUNCTIONS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    ["and", "but", "or", "so", "yet", "for", "nor"]
        .into_iter()
        .collect()
});

/// US/UK spelling pairs `(us, uk)`.
pub static US_UK_PAIRS: LazyLock<Vec<(&'static str, &'static str)>> = LazyLock::new(|| {
    vec![
        ("color", "colour"),
        ("favor", "favour"),
        ("honor", "honour"),
        ("labor", "labour"),
        ("neighbor", "neighbour"),
        ("center", "centre"),
        ("meter", "metre"),
        ("fiber", "fibre"),
        ("organize", "organise"),
        ("recognize", "recognise"),
        ("analyze", "analyse"),
        ("defense", "defence"),
        ("license", "licence"),
        ("traveling", "travelling"),
        ("canceled", "cancelled"),
    ]
});

/// Hyphenation variant pairs `(joined, hyphenated)`.
pub static HYPHEN_PATTERNS: LazyLock<Vec<(&'static str, &'static str)>> = LazyLock::new(|| {
    vec![
        ("email", "e-mail"),
        ("online", "on-line"),
        ("website", "web-site"),
        ("today", "to-day"),
        ("cooperate", "co-operate"),
        ("coordinate", "co-ordinate"),
    ]
});
