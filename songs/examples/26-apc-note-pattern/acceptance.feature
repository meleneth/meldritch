@headless
Feature: Create and assign notes using APC or TUI input
  Background:
    Given the synth from example "01-synth-note-pattern" at 120 BPM and 48000 Hz
    # Bound by crates/meldritch-control/tests/piano_roll.rs, not a Gherkin runner.
    And Lua NEW creates a blank looping two-bar pattern assigned to its track
    And the grid uses eight eighth-note columns per page and chromatic rows from C3

  Scenario Outline: Two presses author pitched events
    When "<surface>" enters grid cells 0 and 9 with velocity 0.8
    And releases both cells
    Then the source notes are exactly
      | step | pitch | onset_frame | duration_frame | velocity |
      | 0    | C3    | 0           | 12000          | 0.8      |
      | 1    | C#3   | 12000       | 12000          | 0.8      |
    And exactly two note insertions are recorded and releases create no edits
    And the TUI and APC projection agree with the assigned pattern
    And the compiled pattern renders finite non-silent split-range-identical audio
    When authored edits are saved as Lua and reloaded into a fresh host
    Then pattern identity, musical notes and track assignment match
    Examples:
      | surface |
      | APC     |
      | TUI     |

  Scenario: Select an existing note without deleting it
    Given cell 0 already contains the authored C3 event
    When APC packets "90 00 7f" then "80 00 00" arrive on the declared Control port
    Then C3 is selected and retained and the release makes no change
    When the authored DELETE modifier is held and C3 is pressed
    Then C3 is removed exactly once and either release order makes no change

  Scenario Outline: Reject invalid or unrelated input
    When the pattern receives "<input>"
    Then no note or accepted fact is created
    And invalid authoring targets have a target-specific diagnostic
    Examples:
      | input                         |
      | wrong device grid press       |
      | wrong channel grid press      |
      | malformed MIDI packet         |
      | insertion into missing pattern|
      | insertion beyond bar end      |
