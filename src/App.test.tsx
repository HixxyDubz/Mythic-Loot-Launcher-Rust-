import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import App from "./App";

describe("Mythic Loot launcher shell", () => {
  it("loads truthful first-run readiness and opens settings", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Mythic Loot Minecraft" })).toBeInTheDocument();
    expect(screen.getAllByText("Choose or detect the game client").length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole("button", { name: /complete setup/i }));
    expect(await screen.findByText("Modpack identity")).toBeInTheDocument();
    expect(screen.getByLabelText("Game or launcher executable")).toBeInTheDocument();
  });

  it("keeps GitHub repository creation behind preflight and preview", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: /publisher/i }));
    expect(await screen.findByRole("heading", { name: "GitHub Publisher" })).toBeInTheDocument();
    expect(screen.getByText("Preflight required")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /prepare release locally/i })).toBeDisabled();
    expect(screen.queryByRole("button", { name: /^create repository$/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /publish github release/i })).not.toBeInTheDocument();
  });
});
