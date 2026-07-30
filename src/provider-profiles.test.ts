import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  connectionTestDraft,
  providerModelOptions,
  unavailableModelValues,
} from "./lib/provider-profiles";
import type { ProviderProfile } from "./model";

describe("provider model profiles", () => {
  it("renders a saved model absent from an authoritative catalogue as unavailable", () => {
    const profiles: ProviderProfile[] = [{
      participantKind: "cursor",
      route: "chat",
      model: "cursor-grok-4.5",
    }];
    const options = providerModelOptions(
      ["cursor-grok-4.5-high"],
      profiles,
      true,
    );
    const markup = renderToStaticMarkup(createElement(
      "datalist",
      undefined,
      options.map((option) => createElement("option", {
        key: option.value,
        value: option.value,
        label: option.label,
      })),
    ));

    expect(markup).toContain('label="cursor-grok-4.5 (Unavailable: not in the current catalogue)"');
    expect(markup).not.toContain('label="cursor-grok-4.5"');
  });

  it("uses the current chat draft for a connection test", () => {
    expect(connectionTestDraft({
      model: " cursor-grok-4.5-high ",
      effort: "high",
    })).toEqual({
      model: "cursor-grok-4.5-high",
      effort: "high",
    });
  });

  it("identifies saved models that a refreshed catalogue cannot execute", () => {
    const profiles: ProviderProfile[] = [{
      participantKind: "cursor",
      route: "chat",
      model: "cursor-grok-4.5",
    }];

    expect(unavailableModelValues(profiles, ["cursor-grok-4.5-high"], true))
      .toEqual(["cursor-grok-4.5"]);
    expect(unavailableModelValues(profiles, [], false)).toEqual([]);
  });
});
