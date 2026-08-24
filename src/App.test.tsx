import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import App from "./App";

describe("Mythic Loot launcher shell", () => {
  it("loads truthful first-run readiness and opens settings", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Mythic Loot Minecraft" })).toBeInTheDocument();
    expect(screen.getAllByText("Choose or detect the game client").length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole("button", { name: /complete setup/i }));
    expect(await screen.findByText("Server identity")).toBeInTheDocument();
    expect(screen.getByLabelText("Game or launcher executable")).toBeInTheDocument();
  });
});
