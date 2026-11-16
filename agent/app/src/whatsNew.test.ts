import { describe, expect, it } from "vitest";
import { PAGE_KEY, type TabStorage } from "./tabs";
import { NEWS, type NewsNote } from "./whatsNewData";
import { dueNote, groups, latestNote, markSeen, noteFor, SEEN_KEY } from "./whatsNew";

function memory(seed: Record<string, string> = {}): TabStorage & { data: Map<string, string> } {
  const data = new Map<string, string>(Object.entries(seed));
  return { data, getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v) };
}
const throwing: TabStorage = {
  getItem: () => {
    throw new Error("blocked");
  },
  setItem: () => {
    throw new Error("blocked");
  },
};

const NOTES: NewsNote[] = [
  {
    version: "2.0.0",
    headline: "A headline.",
    items: [
      { where: "app", text: "An app change." },
      { where: "meter", new: true, text: "A live meter." },
      { where: "meter", text: "A threat bar." },
      { where: "core", text: "An addon change." },
      { where: "palworld", text: "A Palworld change." },
    ],
  },
  { version: "1.9.0", headline: "Earlier.", items: [{ where: "app", text: "Older." }] },
];
const FIRST = NOTES[0]!;

describe("what's new", () => {
  it("ships notes that are well formed", () => {
    expect(NEWS.length).toBeGreaterThan(0);
    for (const n of NEWS) {
      expect(n.headline.trim()).not.toBe("");
      expect(n.items.length).toBeGreaterThan(0);
    }
  });

  it("finds a version's note, and the newest for About", () => {
    expect(noteFor("1.9.0", NOTES)?.headline).toBe("Earlier.");
    expect(noteFor("1.8.0", NOTES)).toBeNull();
    expect(latestNote("1.8.0", NOTES)?.version).toBe("2.0.0");
    expect(latestNote("1.8.0", [])).toBeNull();
  });

  it("groups a note in order, marks a new mod, and keeps other cores' changes to those who have them", () => {
    const wow = groups(FIRST, ["wow"]);
    expect(wow.map((g) => g.label)).toEqual(["Meter · new mod", "Ryna addon", "App"]);
    expect(wow[0]?.lines).toEqual(["A live meter.", "A threat bar."]);
    expect(groups(FIRST, ["wow", "palworld"]).map((g) => g.where)).toEqual(["meter", "core", "palworld", "app"]);
  });

  it("is due once, on the first launch of a version that has a note", () => {
    const used = memory({ [PAGE_KEY]: "status" });
    expect(dueNote("2.0.0", used, NOTES)?.version).toBe("2.0.0");
    expect(used.data.has(SEEN_KEY)).toBe(false); // not seen until closed
    markSeen("2.0.0", used);
    expect(dueNote("2.0.0", used, NOTES)).toBeNull();
    expect(dueNote("2.0.1", used, NOTES)).toBeNull(); // no note for that version
  });

  it("shows nothing on the first launch ever, and marks the version seen", () => {
    const fresh = memory();
    expect(dueNote("2.0.0", fresh, NOTES)).toBeNull();
    expect(fresh.data.get(SEEN_KEY)).toBe("2.0.0");
  });

  it("shows the note to someone who had the app before notes existed", () => {
    expect(dueNote("2.0.0", memory({ [PAGE_KEY]: "about" }), NOTES)).not.toBeNull();
    expect(dueNote("2.0.0", memory({ [PAGE_KEY]: "about", [SEEN_KEY]: "1.9.0" }), NOTES)).not.toBeNull();
  });

  it("survives a store that throws", () => {
    expect(dueNote("2.0.0", throwing, NOTES)).toBeNull();
    expect(() => markSeen("2.0.0", throwing)).not.toThrow();
    expect(dueNote("2.0.0", null, NOTES)).toBeNull();
  });
});
