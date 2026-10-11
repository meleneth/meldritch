@design @headless
Feature: Held onset selects an explicit duration endpoint
  Background:
    Given example 26 contains C3 at step 0 with duration one eighth note

  Scenario: Extend one gate across four steps
    When APC packets "90 00 7f", "90 03 7f", "80 03 00", "90 00 00" arrive in order
    Then exactly one duration edit sets C3 duration to one half note
    And C3 onset remains frame 0 and gate-off is frame 48000 at 120 BPM and 48000 Hz
    And there is one attack with no retrigger at steps 1, 2 or 3
    And release audio follows gate-off using the synth ADSR release
    And TUI duration-span feedback agrees with source events
    When authored edits are saved as Lua and reloaded into a fresh host
    Then C3 has a half-note duration and no held-pad state exists

  Scenario Outline: Clear a lost or obsolete hold
    Given the C3 onset pad is held
    When "<event>" occurs before an endpoint press
    And an old release or endpoint observation arrives
    Then no duration edit is accepted for that obsolete hold
    Examples:
      | event                    |
      | APC disconnect           |
      | MIDI observation overflow|
      | page switch              |
      | pattern switch           |
      | selected note deletion   |

  Scenario Outline: Reject an invalid endpoint
    Given C3 at step 2 is held
    When endpoint "<endpoint>" is selected
    Then the invalid endpoint is diagnosed and the note duration is unchanged
    Examples:
      | endpoint           |
      | C3 step 1          |
      | C#3 step 3         |
      | C3 beyond bar end  |

  Scenario: Transport position does not order held gestures
    Given the C3 onset pad is held while playing
    When transport rewinds and wraps its loop before endpoint step 3 is pressed
    Then the ordered gesture sets one half-note duration exactly once
