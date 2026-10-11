@design @headless
Feature: Author an executable graph from existing modules
  Scenario: Create the supported filtered note patch
    Given a new synth draft with pitch and gate inputs
    And modules osc oscillator, env ADSR, amp VCA, filter low-pass and out audio-output use example 02 defaults
    When the TUI connects and commits these cables
      | from        | to           | signal  |
      | input.pitch | osc.pitch    | pitch   |
      | input.gate  | env.gate     | gate    |
      | osc.audio   | amp.audio    | audio   |
      | env.control | amp.level    | control |
      | amp.audio   | filter.audio | audio   |
      | filter.audio| out.audio    | audio   |
    And assigns the synth to the existing note pattern
    Then the inspector shows exactly these typed modules and cables
    And the source graph validates and compiles using the existing note compiler
    And completed audio is gated, finite, non-silent and split-range identical
    When authored edits are saved as Lua and reloaded into a fresh host
    Then graph identities, defaults, cables, assignment, fingerprint and audio match

  Scenario: Reconnect a removed cable in the draft
    Given the last valid patch is playing
    When filter.audio to out.audio is removed in the draft
    Then the missing output diagnostic prevents publication of the incomplete draft
    When that cable is reconnected and the draft is committed
    Then the inspector and compiled audio again match the supported graph

  Scenario Outline: Reject an invalid graph commit
    Given the last valid patch is playing
    When a graph commit attempts "<invalid>"
    Then the module or endpoint diagnostic explains the failed validation or compilation
    And the authoritative committed source and published audio remain unchanged
    Examples:
      | invalid                              |
      | duplicate osc identity               |
      | cable to nonexistent port            |
      | input.pitch to out.audio             |
      | audio cycle without supported delay  |
      | two oscillators in note compiler path|
