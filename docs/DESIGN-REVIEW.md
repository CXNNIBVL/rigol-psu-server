# Frontend design review

**Status:** §2–§4, §6, §7, §10–§14 implemented; §1 out of scope; §5 partly done;
§9 not started
**Date:** 2026-09-08
**Reviewed:** all four sections at 1440×900, plus 390×844 mobile

---

## Summary

The interface is *correct*. Every safety rule from the plan is visible in it:
unknown readings never render as zero, offline supplies explain themselves,
quarantine and limit violations are named in words, and one accent means
"active" throughout. That is the hard part and it is done.

What it does not yet have is a **point of view**. It currently looks like
Skeleton's Cerberus theme with content in it — which is a reasonable place to
be after three milestones of building behaviour, but it means the thing a user
actually came for (what are my rails doing?) has no more visual weight than the
device path of a USB symlink.

Two findings are severe enough to fix regardless of any redesign: **there are
three accent colours doing two jobs** (§2), and **profile authoring has a dead
end on its main path** (§10).

**§1 (mobile) is out of scope by decision** — this is opened from a desk at a lab
PC. It stays in the document as a record, not as work.

Two further sections were added after the first pass: **§11** sets rules for
dividers and finds that five of the eleven currently in the interface should come
out, and **§12** argues that drawing every collection as the same bordered card
is what makes the bench feel flat.

Issues are grouped by theme and marked **[1]** must fix, **[2]** should fix,
**[3]** worth considering.

---

## 1. Mobile is broken — [out of scope]

**Decided: not a target.** This runs on a lab PC and is opened from a desk. The
findings below are recorded because they are real, not because they are going to
be fixed; if a phone at the bench ever becomes a use, start here.

At 390 px the bench is not usable.

- **The rail takes 100 px of a 390 px viewport, permanently.** A quarter of the
  screen is navigation for a page whose job is showing nine numbers. The rail
  layout is right for a desk monitor and wrong for a phone at the bench.
- **The page overflows horizontally.** The "Panel locked" badge is cut off past
  the right edge of the viewport on every supply header.
- **The three readings collapse into each other:** `UnknownUnknownUnknown`, with
  the vertical rules gone. The `grid-cols-[1fr_auto_1fr_auto_1fr]` has no room
  to resolve, so the columns fuse.
- Supply names wrap mid-phrase ("Simulated / supply 1") against a badge that
  does not wrap with them.

**Proposal.** Below `md`, drop the rail to a bottom bar (Skeleton's `bar`
layout is the same component with a different `layout` prop), stack the readings
vertically with the rules becoming horizontal, and let the supply header wrap as
one block. This is worth doing properly: "a phone at the bench" is a real use
of this thing, and it was the argument for a browser UI over a desktop app in
the first place.

---

## 2. Colour is doing three jobs with three hues — [1]

Right now the interface uses:

| Hue | Where | Means |
|---|---|---|
| Green | Live rail border, selected nav icon, `On` segment | "active" |
| Teal-green | `Online` status badge | "reachable" |
| Blue | `Create`, `Power on` | "this is the primary button" |
| Red | `Delete`, `Power off` | "destructive" |
| Amber | `Offline`, `Unknown`, violation banners | "attention" |

Two problems.

**Blue means nothing.** It is Skeleton's default primary, applied because a
button needed a colour. `Power on` is the single most consequential control in
the application and it is currently the same blue as `Create` on a preset form.
Either it earns the active-green (it *is* the thing that makes rails active) or
it earns its own treatment — but not the default.

**Green is overloaded.** `Online` (a supply is reachable) and a live rail (a
channel is energised) are different facts in adjacent green-family colours. A
reachable supply with everything off is a completely different situation from an
energised rail, and they should not read as the same signal.

**Proposal.** Collapse to: **energised/active = green**, **attention = amber**,
**destructive = red**, and everything else neutral. Make `Online` neutral —
"online" is the unremarkable case, and the badge only needs to be loud when it
says something else. That also removes the blue entirely.

---

## 3. The readings are not the hero — [2]

The dashboard is described in the plan as "the product", but on screen:

- A live 3.300 V reading is set in `text-lg` — the same family, weight and
  roughly the same size as the label "Limits 30.000 V / 3.000 A" beneath it.
- Off channels occupy identical space and structure to live ones. On a
  three-supply bench most channels are off most of the time, so the majority of
  the screen is `0.000 V / 0.000 A / 0.00 W` repeated six times.
- There is no bench-level answer to "what is drawing power right now" — you get
  it by scanning nine cards.

**Proposal.** Let the live rails dominate: larger numerals, and collapse an off
channel to a single quiet row (name, setpoint, an On control) that expands when
it matters. Consider a one-line bench summary — rails live, total draw — as the
first thing on the page.

---

## 4. Numbers are set as text, not as data — [2]

- **Centred.** Readings are centre-aligned, so decimal points do not line up
  between CH1, CH2 and CH3. `tabular-nums` is set, which fixes digit width and
  is then undone by the centring. Right-aligning (or decimal-aligning) makes a
  column of currents comparable at a glance.
- **Fixed 3 decimal places regardless of magnitude.** `30.000 V` for a limit,
  `0.00 W` for nothing. Precision should follow the quantity: a 30 V limit is
  `30 V`, a measured rail is `3.300 V`.
- **Units repeated in every cell** rather than sitting in the column heading.
- The same face is used for numbers and for prose. This is a measurement
  instrument; the numerals could carry the identity (see §9).

---

## 5. Copy — [2]

- **"Set" is ambiguous** and the button is disabled unless *both* fields are
  filled — so there is no way to change only the current limit. Either field
  alone should be a valid edit; the label should say what it does
  ("Apply", or better, make the fields commit on blur/Enter and drop the button).
- **Placeholders are used as labels.** The setpoint inputs show `V` and `A` as
  placeholder text, which disappears as soon as you type — leaving two unlabelled
  boxes containing numbers. There are `aria-label`s, so screen readers are fine;
  sighted users are not.
- **`·` meta strings in four places** — `psu1 · /dev/rigol-psu1`,
  `Limits … · setpoint …`, `5.000 V / 1.000 A · OVP … · OCP …`, and the rail
  lines in profile cards. This is a generic pattern that reads as filler; each
  case is really two or three separate facts that could be laid out as such.
- **`/dev/rigol-psu1` on the bench page** is deployment detail. It belongs on
  Health, next to the serials, not on the screen someone uses to power a board.
- **"Presets" and "Profiles"** are near-synonyms sitting adjacent in the rail.
  The distinction is real and load-bearing (one channel's settings vs a device's
  rails) but the names do not carry it. Worth renaming — for instance **Rails**
  and **Devices** — which also matches how someone at the bench talks.

---

## 6. Destructive actions are the loudest thing on screen — [2]

On the preset library, three filled-red `Delete` buttons are the highest-contrast
elements on the page, competing with the content. `Delete` should be quiet until
sought — a text button, or behind an overflow menu — with red reserved for
confirmation.

Related: **deleting a preset in use fails, and the only way to find out is to
try.** The server refuses it correctly and explains itself, but the UI could
show "used by 2 profiles" on the row and disable the control, turning a
rejection into information.

---

## 7. Two risk models for the same action — [2]

Energising a rail from the **bench** takes one click, with no confirmation.
Energising the same rail via a **profile** takes two deliberate steps (apply,
then power on) by design.

Both may be right — a single channel is a smaller commitment than a whole device
— but the inconsistency should be a decision rather than an accident. If the
single-channel path is fine as it is, the profile flow's second step needs a
clearer justification in the UI than it currently has.

Also: **`Power on` is disabled until setpoints are applied, and the reason lives
only in a `title` attribute** — invisible on touch, and to anyone not hovering.
A disabled control with no visible reason reads as a broken one.

---

## 8. Layout and containment — [3]

- **`Edit` / `Delete` sit outside the profile card they act on**, floating below
  its border. They belong inside it.
- **Form fields stretch to their container** — the profile name input is ~630 px
  wide. Field width is a useful signal about expected input; a name is not 630 px
  of name.
- **The presets page leaves a large empty region** on the right with only three
  presets, because the two-column split is fixed rather than responsive to
  content.
- **Health rows are ~1300 px wide carrying two facts**, with a large void below.
  The page also duplicates most of the bench's supply headers; its unique content
  is the serial comparison. It may not need to be a section of its own — it could
  be a detail view opened from a supply.
- **The `Order` / `Dwell` / `Energise` / `Remove` row** in the profile editor is
  cramped and its baselines do not align.

---

## 9. There is no visual identity — [3]

This is the largest opportunity and the least urgent item.

The interface currently reads as "Skeleton Cerberus with content". Nothing about
it says *bench instrument* rather than *admin dashboard*: `system-ui` throughout,
uniform card rounding, uniform borders.

The subject matter offers a strong, specific direction. This is a rack of
laboratory power supplies — the vernacular is precision instrumentation:
engraved panel lettering, seven-segment and dot-matrix displays, silkscreen on
PCB, banana-jack colour coding, the yellow-on-black of a scope trace.

**A direction worth considering** (proposal, not a decision):

- **Numerals carry the identity.** A single distinctive face for measured values
  — something with true tabular figures and an instrument feel — set noticeably
  larger than everything else, with units in a smaller optical size. Prose stays
  in one neutral, highly legible sans. Two families, sharply distinct in role.
- **The panel as the container.** Supplies are physical objects; a supply's three
  channels could read as one instrument face rather than three floating cards,
  which also solves the "nine identical cards" problem in §3.
- **Restraint everywhere else.** If the numbers are the hero, borders, badges and
  buttons should recede. Spend the boldness in one place.

I would not start here. Fix mobile (§1) and the colour system (§2) first, because
those are defects; §3–§5 are the ones that would most change how the thing feels
to use; §9 is what would make it feel like *yours*.

---

## 10. Profiles and presets: the schema is the interface — [1]

This is the weakest part of the application, and the problem is structural
rather than cosmetic.

### The task and the model have been swapped

Someone's actual task is *"power my Mainboard rev C"*. To do it they must:

1. go to **Presets**, create `core 1v2` — a named thing that does nothing on its own;
2. go to **Presets** again, create `io 3v3`;
3. go to **Profiles**, create `Mainboard rev C`;
4. add a rail, choose a channel, choose a preset, type an order, type a dwell, tick a box;
5. repeat per rail.

That is the database schema, rendered as forms, in the order the tables were
normalised. The preset/profile split is a *storage* decision — it exists so two
devices can share settings without duplicating them — and it has been promoted
into a *navigation* decision, as two sibling sections in the rail. Most people
building a profile will create presets used exactly once; the interface is
optimised for the minority case.

### The dead end

**A preset cannot be created where it is needed.** In the profile editor the
preset dropdown lists what already exists. If the rail you are adding needs
settings you have not created yet, you must leave the editor — losing the
profile you were part-way through, since there is no draft — go to Presets,
create it, come back, and start again.

This is the single worst moment in the application, and it is on the main path
of the feature the project exists for.

**Fix, cheap version:** a "New preset…" option in that dropdown that opens a
small inline form, creates the preset, and selects it. No model change, no
navigation, no lost work.

**Fix, honest version:** let a rail hold values directly, with a preset as an
*optional* extraction ("save these settings as a preset"). That inverts the
forced order — you describe the device, and reuse becomes something you opt into
once you notice it. It needs a schema change (bindings gain optional inline
values), so it is a real piece of work rather than a UI tweak. I would take the
cheap version first and see whether the honest one is still wanted.

### The sequence is invisible

Order and dwell describe a **power-up sequence** — CH1 comes up, wait 150 ms,
CH2 comes up. That is the one kind of content where numbered steps genuinely
belong. It is currently two number inputs in a cramped row, and on the profile
card it renders as a table:

```
1. psu1/CH1        core 1v2 · 1.200 V / 1.000 A · wait 150 ms
2. psu1/CH2                     io 3v3 · 3.300 V / 1.000 A
```

Two problems beyond it being a `·` meta string. **"wait 150 ms" sits on rail 1
but describes the gap between rail 1 and rail 2** — read literally it says rail
one waits before coming up, which is the opposite of what happens. And a
sequence rendered as a right-aligned column reads as a list of attributes, not
as something with a direction in time.

**Proposal:** show it as the sequence it is — steps down the page, with the
dwell as a labelled gap *between* steps rather than a property of one:

```
  ①  psu1 · CH1      1.200 V   1.000 A
      │  wait 150 ms
  ②   psu1 · CH2      3.300 V   1.000 A
```

**Order should be positional, not typed.** A free number field lets you enter
`1, 1, 1`, and the model happily stores ties whose resolution is then incidental.
Reordering should be drag or up/down, with the numbers derived.

### A profile card does not say what is happening

The most useful thing a profile could tell you is its **current state**: are
these rails staged, live, or off right now? The card shows none of it. After
pressing *Apply setpoints*, the only visible change is that *Power on* stops
being disabled — the staged values themselves are invisible unless you navigate
to Bench and read them off the channels.

A profile is a claim about several rails across possibly several supplies. It
should show whether reality currently matches that claim.

### Smaller things on these two screens

- **`Edit` / `Delete` sit outside the card they act on**, floating below its
  border and not aligned to it.
- **Deleting a profile is immediate and unconfirmed.** A profile is minutes of
  work; a misclick should not be enough.
- **Cards are wildly ragged** in the two-column grid, and a profile without a
  description has a different anatomy from one with — the layout changes shape
  depending on content.
- **A preset is unreadable in isolation.** `core 1v2 — 1.200 V / 1.000 A · OVP
  1.500 V · OCP 1.200 A` does not say where it is used or whether it is safe on
  the channels it is bound to. Showing "used by 2 profiles" on the row would also
  turn the delete-refusal (§6) from a rejection into information.
- **`Power on` is the most consequential control in the application** and is
  currently the same blue as `Create` on a preset form (§2), sitting third in a
  row of three same-sized buttons.

### Naming, again

**Presets** and **Profiles** are near-synonyms in English, sit adjacent in the
rail, and are the two things a new user must distinguish immediately. The
distinction is real — one channel's settings versus a device's rails — but the
words do not carry it.

**Correction on implementing this.** The review first proposed **Rails** and
**Devices**. That is wrong: `device` already means *a power supply* throughout
the API and the domain crates (`DeviceView`, `/api/devices`), so calling the
profiles section "Devices" would collide with the vocabulary the rest of the
system uses. Renaming one half was enough — presets are now **Settings**
("Rail settings" on the page), which reads as subordinate to Profiles rather
than parallel with it, and does not fight any existing term.


---

## 11. Dividers: rules for where they earn their place — [2]

The dividers currently in the interface were added at every seam that could take
one. Several of them decorate rather than inform, which is the opposite of what a
structural device is for. There are eleven divider usages; I would keep four.

### The principle

**A rule stands in for space you do not have.** Where there is room to separate
two things with whitespace, whitespace is better — it separates without adding a
mark. Rules are for dense regions, and for boundaries where the content genuinely
changes character.

Five working rules:

1. **Don't rule a boundary that is already stated.** A heading plus generous
   space already separates two sections. Adding a rule makes it the third signal
   saying the same thing.
2. **A bordered container gets at most one internal rule.** The border is already
   a rule. A card with an outline, an internal horizontal rule and two vertical
   rules is three levels of ruling inside one small box.
3. **Never rule between a thing and its own attribute.** A rule says "these are
   different kinds of thing". A status badge is an attribute of the supply it
   sits beside, not its peer.
4. **Vertical rules separate parallel quantities, never sequential content.**
   Voltage, current and power are three simultaneous readings of one channel, so
   a rule between them is honest. A sequence of steps gets space or a connector,
   because a rule implies the items are side by side in kind rather than ordered
   in time.
5. **A rule must survive reflow.** A vertical rule inside a grid that collapses
   becomes nothing and takes its separation with it — which is how the three
   readings fused into `UnknownUnknownUnknown` at 390 px (§1). Mobile is not a
   target, but a narrowed desktop window reaches the same place, so the rule
   stands on its own.

### Audit of what is there now

| # | Where | Verdict |
|---|---|---|
| 1 | Vertical rules between Voltage / Current / Power | **Keep** — parallel quantities, tight space, proven to fuse without one |
| 2 | Rule between a channel's readings and its controls | **Keep** — genuine change of character: what it is doing, what you can do |
| 3 | Rule under the page header | **Keep** — one mark establishing the content area |
| 4 | Rule in the preset form, before the actions | **Keep** — fields and commitment are different acts |
| 5 | Rule between one supply and the next | **Cut** — heading, spacing and a card group already say it |
| 6 | Rule under each supply's header | **Cut** — doubles up with #5; between them the eye sees rules everywhere |
| 7 | Vertical rule between supply name and status badge | **Cut** — the badge belongs to the name; rule 3 |
| 8 | Vertical rule in the health row header | **Cut** — same reason |
| 9 | Rule under the health row header | **Cut** — the row is a bordered card already; rule 2 |
| 10 | Rule in the profile card before its actions | **Reconsider** — defensible, but see §12: profile cards should become list rows, which changes the question |
| 11 | Rule under the rail wordmark | **Reconsider** — the rail has its own background; the mark may be doing nothing |

Cutting five of eleven is the point. Rules read as information only while they are
scarce.

---

## 12. Collections: the same card for everything — [1]

Every collection in the interface is drawn the same way: a grid or list of
same-weight bordered boxes, with the same radius, the same border colour and the
same padding — supplies, channels, profiles and presets alike.

The consequence is that **nesting level and kind are indistinguishable**. A
channel inside a supply looks like a profile inside a list looks like a preset in
a row. There is no visual grammar telling you what contains what.

Four specific costs.

### Nine channels of equal weight

Three supplies × three channels = nine identical cards. At any moment perhaps two
are live; the rest are the majority of the screen showing `0.000 V / 0.000 A /
0.00 W`. A collection should say something about itself before you read its
members — how many rails are live, what the bench is drawing — and should give
its interesting members more weight than its idle ones.

### Invariant labels repeated nine times

Each card carries its own "Voltage / Current / Power" headings. Those are column
headings for a table, printed once, not per-item labels printed nine times. They
are also the kind of label that exists because a value looked bare, rather than
because a reader needed it.

### Grouping follows the wiring, not the task

Channels are grouped by which supply they are plugged into, because that is how
the hardware is arranged. But the unit someone thinks in is *the rails powering
my board*, which may span two supplies. Profiles express that grouping and live
in a different section entirely, so the bench cannot show it.

### Ragged grids

Profile cards sit in a two-column grid with wildly different heights, because
their content length varies — a profile with a description has a different
anatomy from one without. Presets sit in a right-hand column that leaves a large
void when there are three of them.

### Proposal: let the container follow the thing

A DP832 is a physical object: one face, one display, three outputs. The three
channels are not three cards that happen to be near each other — they are three
rows of one instrument. Drawing them that way solves the repetition, the
alignment and the hierarchy at once.

```
  Bench Left                      psu1 · online · panel locked
  ─────────────────────────────────────────────────────────────
              Voltage     Current     Power     Limit
  ▸ CH1        3.300 V     0.330 A    1.09 W    5.5 V / 2 A   ●On
    CH2          —           —          —        15 V / 3 A    Off
    CH3          —           —          —         5 V / 3 A    Off
```

- Column headings **once per supply**, not once per channel.
- Numbers in a column, decimal-aligned, so currents are comparable down the page.
- An idle channel is a quiet row with an em dash — not a fabricated `0.000 V`,
  which also settles §4's precision problem.
- The live row carries the accent; the supply as a whole can show it is live.
- The vertical rules between readings become unnecessary: a table has columns.
- Four supplies still fit on one screen.

Then let the other collections take the shape their content wants:

| Collection | Shape | Why |
|---|---|---|
| Supplies | Panel | A physical object with a face |
| Channels | Rows within a panel | Three parallel things, compared |
| Profiles | List rows that expand in place | Variable length; expanding shows the sequence (§10) without leaving the page |
| Presets | Dense table — name, V, A, OVP, OCP, used by | The smallest unit, least interesting alone, most useful compared |

The rule underneath: **weight follows how much the thing matters and how much it
varies.** A card is a strong container and should be spent on something that
earns it, not applied to every level because it is the component that exists.

This also removes the ragged-grid problem for free — a list does not need its
items to be the same height.


---

## 13. Later requests, implemented

Added after the review and built alongside it.

**Create and edit happen in a dialog.** Both editors previously pushed the list
down the page, so you lost sight of what you were editing against. They are now
modals built on the **native `<dialog>` element** rather than Skeleton's headless
Dialog anatomy — the anatomy ships no styles at all (there is no `dialog.css` in
`skeleton-common`, unlike every other component), while the base package styles
`<dialog>` directly. The platform then supplies focus trapping, Escape-to-close
and an inert background rather than leaving them to be reimplemented.

**Lists are searchable and scroll within themselves.** Profile search matches the
slot, channel and settings names as well as the profile's own words, so "psu3"
or "3v3" finds the devices that use them — which is how you look for a profile
when you remember the hardware but not the name. Each list scrolls in its own
region so the search box stays reachable however many there are.

**Create, edit and delete are icons**: plus, pencil, bin, colouring green, amber
and red on hover and focus. Two constraints came with that:

- They are **colourless at rest**. Three lit buttons per row would make the
  actions louder than the content they act on, and §6's point about a permanently
  red bin dominating the page applies to a red bin icon just as much.
- An accessible name is **required, not optional**. There is no visible text to
  fall back on, so every one carries an `aria-label` that is also its tooltip,
  and a browser test asserts it.

One note on the palette: green now also appears as the hover state for "new",
which slightly overloads the accent that otherwise means *energised*. It reads
fine because it is a transient hover on a control that touches no hardware, but
it is worth knowing the accent is doing one more job than §2 assigned it.

---

## 14. Health folded into Bench, and the subtractions around it

**The Health section is gone.** Its only unique content was the configured and
reported serial numbers; status, offline reason, quarantine explanation, panel
lock and limit violations were all already on Bench. A whole section existed to
hold two strings, and every visit to it meant leaving the thing it described.

What replaced it:

- **Metadata is a collapsible on the supply itself.** Slot, device node and
  serial, closed by default — reference facts needed rarely and needed precisely
  when something is wrong. The configured serial appears only when it disagrees
  with the reported one; printing both when they match says nothing. The
  quarantine banner still names both inline, which is the case that matters.

  Built first as an absolutely positioned panel, which the table's own
  horizontal scroll container clipped — it could not be read at all. In flow
  now, expanding the panel rather than floating over it.
- **The bench header speaks only when something needs attention.** It first
  carried a live-rail count and a running total as well. Neither prompted any
  action — the live rails are already marked in the table and the draw per
  channel is a column of its own — so both were headline space spent restating
  what was visible underneath. Removed; the header now appears only when there
  is a supply to look at or the connection is down.
- **The attention count moved to Bench**, which is where the supply in question
  lives.

Five subtractions went with it, all removing things rather than adding:

1. **Setpoints are text until you ask to change them.** Nine pairs of
   permanently open inputs — eighteen borders — out-shouted the readings, which
   are the point of the table. A pencil opens the one row being edited.
2. **The Limit column is gone.** It repeated the same pair nine times down the
   page; the limit now appears beside the inputs while editing, where it is
   actually constraining something.
3. **An offline supply carries no controls.** Nine greyed-out inputs and toggles
   are nine things that cannot be done, and the banner already says why.
4. **The per-supply summary has one shape**, whatever the state. It previously
   rendered three different strings depending on what was happening.
5. **The `·` is gone from that summary.** §5 called out middle-dot meta strings
   as template chrome and this review then introduced one; parallel facts are
   separated by rules, per §11's own rule about vertical rules.

Connection status moved into the rail's footer. It is chrome rather than
content, and belonged with the frame rather than inside whichever view happened
to be open.

---

## 15. One control per bit

A channel's row carried a status dot beside a two-segment On/Off control. Both
said the same thing — once as a reading, once as a choice — and the dot was
pure duplication of the control immediately beside it.

They are now a single power button, using IEC 5009, the symbol already on every
instrument in the rack. Lit green when the rail is energised; outlined and quiet
when it is not.

**Not red for off.** Red on a power supply means a fault or a trip, and it is
already this interface's destructive colour. A rail that is simply off is the
safe resting state, so a bench at rest would have shown a wall of red buttons —
crying wolf, and teaching everyone to discount the colour that matters. Neutral
is the honest reading of "nothing is happening here".

Two details the change forced, both improvements:

- **The disabled direction is now precise.** The segmented control had to
  disable its "on" half for a quarantined supply. A single button disables only
  when a press *would energise* — so a quarantined rail that is already live can
  still be switched off, which is the rule quarantine has always had.
- **`aria-pressed` rather than a label that flips.** A toggle button that
  renames itself between "On" and "Off" is ambiguous about whether it reports
  the current state or the action a press performs. The name stays constant and
  the pressed state reports; `mixed` covers a rail whose state is unknown.

The first attempt filled the off state tonally, which on this background was
close to invisible — the control stopped reading as a control. Outlined instead:
quieter than a live rail, still obviously pressable.

**Column order.** The power button sits with the channel it belongs to, which
also puts every rail's state in one scannable column down the left edge — the
fastest way to answer "what is live" without reading anything. Setpoints moved
to the far right: they are what you *change*, and they were sitting between the
measured columns and the control, splitting the reading from the row it belongs
to.

Column widths are fixed rather than sized to content. Each supply's table was
laying itself out independently, so a badge on one channel shifted that panel's
numbers relative to the panel above it, and comparing a current across supplies
meant re-finding the column each time.

---

## 16. Picking a channel is not a list

The rail editor chose its channel from a native dropdown: nine options, each
repeating its supply's name and its limits, and one more line for every channel
on every supply added.

Choosing a rail is a **two-dimensional** decision — which supply, then which
channel — and a flat list throws that structure away. It is now laid out the way
the rack is: a row per supply, its three channels as chips.

- The supply's name is written **once per row**, not once per option.
- Limits are stated **once, for whatever is selected**, rather than on all nine
  lines.
- A channel another rail already claims is **visibly unavailable**, so the
  double-binding rule is enforced by the control instead of explained by a
  warning after the fact.
- An unreachable supply is marked but still selectable — a profile may name a
  supply that happens to be unplugged, and applying it is what refuses.
- Six supplies are six short rows rather than eighteen lines of scrolling.

Adding a rail now picks the first *free* channel too. Defaulting to the first
channel outright meant every rail after the first landed on one already in use
and warned about it before the user had done anything.

The settings dropdown was left as a plain select. Its entries are distinct named
things rather than repetitions of one another, so a list is the right shape for
it; if it grows enough to need searching, that is a separate change.

---

## What is working, and should survive any redesign

- **Unknown is never a number.** The rule holds everywhere, including the
  segmented control, which shows no selection rather than defaulting to "Off".
- **Exceptional states are named in words**, and never dimmed into the
  background.
- **The rail's attention count** surfaces a supply dropping out from any section.
- **Applying a profile cannot energise anything** — and the UI makes the two
  steps visibly separate.
- **Errors say what happened and what the limit was**, rather than "invalid
  input".

---

## Awaiting confirmation

Nothing above has been changed.

Recommended order:

1. **§10's dead end** — the inline "New preset…" in the profile editor. Smallest
   change here with the largest effect, and it removes a trap from the main path.
2. **§2 colour** — a defect rather than taste.
3. **§12's supply panel** — channels as rows of one instrument. This subsumes
   §3 (hierarchy), §4 (numeral alignment and precision) and half of §11
   (the vertical rules stop being needed), so it is better value than fixing
   those three separately.
4. **§11's cuts** — five dividers out. Ten minutes, and it makes the remaining
   ones mean something.
5. **§10's sequence view** and the profile card's live state — this is what makes
   profiles feel like a feature rather than a form over two tables.
6. **§5, §6** — copy, and quiet destructive actions.
7. **§9** — the visual identity, and only after a conversation about direction.

The one item I would not start without your decision is §10's "honest version"
(rails holding values directly, presets as optional extraction). It is the right
model, but it changes the schema, and the cheap version may well make the
question moot.
