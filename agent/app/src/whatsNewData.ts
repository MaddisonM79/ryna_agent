// The notes the app shows after an update. Written by scripts/whats-new.mjs from whats-new.json at
// the repository root: change that file and run the script, never this one.

export interface NewsItem {
  where: string;
  text: string;
  new?: boolean;
}

export interface NewsNote {
  version: string;
  headline: string;
  items: NewsItem[];
}

export const NEWS: NewsNote[] = [
  {
    "version": "next",
    "headline": "Live meters and a threat bar, read from the game itself.",
    "items": [
      {
        "where": "meter",
        "new": true,
        "text": "A live meter: damage, DPS, healing, HPS and more, to two decimals."
      },
      {
        "where": "meter",
        "text": "Click a meter's first line to choose what to show and which fight."
      },
      {
        "where": "meter",
        "text": "Up to four windows, locked together if you like."
      },
      {
        "where": "meter",
        "text": "Hover a row to see what that player used."
      },
      {
        "where": "meter",
        "text": "A threat bar for your group, orange when someone is about to pull."
      },
      {
        "where": "meter",
        "text": "Announce a window to party, raid, guild or say."
      },
      {
        "where": "fights",
        "new": true,
        "text": "Your recent fights in Ryna's window: who struck first, who died when, damage done and taken."
      },
      {
        "where": "core",
        "text": "Mods are now named \"Chronicle by Ryna\", \"Prices by Ryna\" and so on."
      },
      {
        "where": "core",
        "text": "This note: once after each update. /ryna news shows it again."
      },
      {
        "where": "app",
        "text": "An older addon in your game folder is updated when the app starts."
      }
    ]
  }
];
