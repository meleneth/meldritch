@design @headless
Feature: Author physical mappings to existing parameters
  Background:
    Given a temporary copy of example "22-midi-soft-takeover"
    And the TUI authors linear CC48 cutoff range 80 to 6000 and CC49 lane-level range 0 to 1

  Scenario: Acquire and edit each declared target
    When CC48 crosses its projected target then moves to raw value 127
    And CC49 crosses its projected target then moves to raw value 0
    Then source cutoff is 6000 Hz and the mapped lane level is 0
    And each acquired movement records exactly one command
    And that lane renders silence and other lanes retain their source settings
    When authored edits are saved as Lua and reloaded into a fresh host
    Then both mappings, curves, target identities and authored values match

  Scenario: Remapping rearms pickup
    Given CC48 is acquired
    When the TUI remaps CC48 to another existing low-pass target
    And a distant physical observation arrives
    Then the mapping inspector shows the new target waiting for pickup
    And no value edit, render request or accepted musical fact is created

  Scenario Outline: Preserve mappings on rejection
    When a mapping edit attempts "<invalid>"
    Then the mapping diagnostic identifies the invalid binding or target
    And previous source, mappings, journal and publication remain unchanged
    Examples:
      | invalid                         |
      | nonexistent target              |
      | descending cutoff range         |
      | nonfinite range                 |
      | duplicate CC in the same context|
