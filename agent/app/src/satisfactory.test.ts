import { describe, expect, it } from "vitest";
import type { CoreState, CoreView } from "./api";
import { corePanel, coreWords, describeCoreEvent, portError, runningCores, sidecarProblem } from "./coreWords";
import { headerState } from "./describe";
import { pages } from "./tabs";

function satisfactory(state: CoreState, over: Partial<CoreView> = {}): CoreView {
  return {
    id: "satisfactory",
    name: "Satisfactory",
    version: "0.4.1",
    kind: "builtin",
    switchable: true,
    relay: { port: 8080, allowChanges: false },
    rows: [],
    state,
    on: false,
    platforms: [],
    lastError: null,
    ...over,
  };
}

const wow: CoreView = {
  ...satisfactory({ is: "running" }),
  id: "wow",
  name: "World of Warcraft: Forever",
  switchable: false,
  relay: null,
};

describe("the Satisfactory core's page", () => {
  it("is ready to turn on, in its own words", () => {
    expect(corePanel(satisfactory({ is: "ready" }))).toEqual({
      state: "ready",
      tone: "",
      title: "Satisfactory is ready to turn on",
      text: "Your ryna.io account hosts a Satisfactory factory. Turning this on lets ryna.io read the factory through this computer while Ryna is open. Nothing is read until then.",
      progress: null,
      action: { label: "Turn on Satisfactory", does: "turnOn" },
    });
  });

  it("is off, or stopped with another computer's takeover and Start again", () => {
    expect(corePanel(satisfactory({ is: "off" }))?.title).toBe("Satisfactory is off on this computer");
    expect(
      corePanel(satisfactory({ is: "stopped", message: "Another computer is now the factory's link to ryna.io.", fix: null })),
    ).toMatchObject({
      tone: "error",
      title: "Satisfactory stopped",
      text: "Another computer is now the factory's link to ryna.io.",
      action: { label: "Start again", does: "start" },
    });
    expect(
      corePanel(satisfactory({ is: "stopped", message: "This account doesn't host the factory.", fix: null }))?.text,
    ).toBe("This account doesn't host the factory.");
    expect(corePanel(satisfactory({ is: "running" }))).toBeNull();
  });

  it("has its own switch words, and Palworld keeps its", () => {
    expect(coreWords("satisfactory").switchLabel("Satisfactory")).toBe("Let ryna.io read the factory through this computer");
    expect(coreWords("palworld").switchLabel("Palworld")).toBe("Sync Palworld on this computer");
    expect(coreWords("palworld").readyText("Palworld")).toContain("downloads it");
  });

  it("is a sidebar row only when provisioned", () => {
    expect(pages([wow]).map((p) => p.id)).not.toContain("core:satisfactory");
    expect(pages([wow, satisfactory({ is: "ready" })]).map((p) => p.id)).toContain("core:satisfactory");
  });

  it("takes a port from 1 to 65535", () => {
    expect(portError("8080")).toBeNull();
    expect(portError("1")).toBeNull();
    expect(portError("65535")).toBeNull();
    expect(portError("0")).toBe("A port is between 1 and 65535.");
    expect(portError("65536")).toBe("A port is between 1 and 65535.");
    expect(portError("80a")).toBe("A port is a number.");
    expect(portError("")).toBe("A port is a number.");
  });
});

describe("the relay's activity lines", () => {
  it("say the link and the factory changing, named", () => {
    expect(describeCoreEvent("Satisfactory", { event: "connected" })).toEqual({ text: "Satisfactory: linked to ryna.io.", level: "info" });
    expect(describeCoreEvent("Satisfactory", { event: "disconnected", why: "no answer from ryna.io for 75 s" })).toEqual({
      text: "Satisfactory: the link to ryna.io dropped (no answer from ryna.io for 75 s). Reconnecting.",
      level: "error",
    });
    expect(describeCoreEvent("Satisfactory", { event: "frmAnswering", port: 8080 })?.text).toBe(
      "Satisfactory: the factory is answering on 127.0.0.1:8080.",
    );
    expect(describeCoreEvent("Satisfactory", { event: "frmNotAnswering", port: 8080 })?.level).toBe("error");
    expect(describeCoreEvent("Satisfactory", { event: "writeRefused", path: "/setEnabled" })?.text).toContain(
      "changes aren't allowed on this computer",
    );
  });
});

describe("the header counts the relay like any core beside WoW", () => {
  const base = { signedIn: true, watching: true, lastRun: null, lastError: null, deviceKeyError: null, accounts: [] };
  it("counts it running and names its stop", () => {
    expect(runningCores({ watching: true, cores: [wow, satisfactory({ is: "running" })] })).toBe(2);
    expect(headerState({ ...base, watching: false, cores: [wow, satisfactory({ is: "running" })] }).state).toBe("connected");
    expect(
      sidecarProblem([satisfactory({ is: "stopped", message: "Another computer is now the factory's link to ryna.io.", fix: null })]),
    ).toBe("Satisfactory: Another computer is now the factory's link to ryna.io.");
  });
});
