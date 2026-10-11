@accept @headless
Feature: Author an executable graph from existing modules
  Scenario: Create the supported filtered note patch
    Given a replacement synth draft under the existing track identity with pitch and gate inputs
    And modules source oscillator, contour ADSR, gain VCA, tone low-pass and out audio-output use inspectable supported defaults as authored in keyboard-script.txt
    When the TUI connects and commits these cables
      | from            | to            | signal  |
      | input.pitch     | source.pitch  | pitch   |
      | input.gate      | contour.gate  | gate    |
      | source.audio    | gain.audio    | audio   |
      | contour.control | gain.level    | control |
      | gain.audio      | tone.audio    | audio   |
      | tone.audio      | out.audio     | audio   |
    And retains the synth assignment to the existing note pattern
    Then the inspector shows exactly these typed modules and cables
    And the source graph validates and compiles using the existing note compiler
    And completed audio is gated, finite, non-silent and split-range identical
    When authored edits are saved as Lua and reloaded into a fresh host
    Then graph identities, defaults, cables, assignment, fingerprint and audio match

  Scenario: Reconnect a removed cable in the draft
    Given the last valid patch is playing
    When the base filter.audio to out.audio cable is removed in the draft
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
      | duplicate osc identity in the base   |
      | cable to nonexistent port            |
      | input.pitch to out.audio             |
      | audio cycle without supported delay  |
      | two oscillators in note compiler path|
