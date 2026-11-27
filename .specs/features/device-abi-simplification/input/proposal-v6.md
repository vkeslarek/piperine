# The Piperine device ABI, simplified: proposal v6

This is the final text of the design review of Piperine's device ABI, and the document submitted for
the user's approval. It stands alone. Nothing in the repository was edited and nothing was compiled to
write it. Every statement about the present state of the code cites `origin/main` as of the review
snapshot. The history of the review is in a separate file, `proposal-v6-review-history.md`; the
appendix keeps only its scorecard.

The user made two complaints, and the design answers both. Of the present ABI: concepts were not
separated, names did not match what things do, and it was unclear where each thing goes. Of an earlier
version of this proposal, which separated everything: *"an absurd quantity of things"*.

Path shorthand used in citations:

| Shorthand | Path |
| --- | --- |
| `abi/` | `crates/piperine-abi/src/` |
| `an/` | `crates/piperine-analog-solver/src/device/` |
| `dg/` | `crates/piperine-digital-solver/src/` |
| `sv/` | `crates/piperine-solver/src/` |
| `plan/` | `crates/piperine-plan/src/` |
| `tp/` | `crates/piperine-transpile/src/` |
| `spec/` | `docs/spec/`, the specification of the modelling language |
| `abi.md` | `.specs/features/device-abi/abi.md`, the present specification of the ABI |
| manual | the ABI manual of 2026-11-26 (`abidoc/index.html`), cited by section |

**Section 0** is the budget and the map: what there is to learn, in a small core and four closed
catalogues. **Part I** explains the problem, the design and its cost. **Part II** is the normative
specification. **Part III** holds the tables: the mapping from the present contract, the coverage of
every present capability, and the divergences between documentation and code. **Part IV** lists the
decisions that only the user can take.

---

# 0. The concept budget

## 0.1 The core and the catalogues

The ABI is presented on two levels, as the Java Virtual Machine Specification is: a few *kinds* of
thing, and under each kind a closed catalogue. A reader who holds the nine kinds is oriented, and can
place anything else by looking it up.

**The core: nine kinds.**

| # | Kind | What it is |
| --- | --- | --- |
| 1 | **Library and device type** | What is loaded: a versioned collection of device types; each device type is four entry points and a default shape. |
| 2 | **Instance** | One occurrence of a device type; to the solver, an opaque pointer with no memory the solver must know of. |
| 3 | **Shape** | The complete, immutable description of one instance: eleven tables. |
| 4 | **Layout** | The only description of bytes. |
| 5 | **Pin** | The instance's end of one net of the circuit. |
| 6 | **Linkage** | The field of a pin that says which net it is. |
| 7 | **Analog table** | A place where a result of analog code goes. |
| 8 | **Code** | A function (pure, called when a result is needed) or a reaction (called when an event fires; it makes commits through its commit sites). |
| 9 | **Aspect** | The one notation by which code names what it reads and writes: a facet of a table, and an index. |

**The catalogues: four, all closed.**

| Catalogue | Entries | Count |
| --- | --- | --- |
| Pins | continuous pin (on an *unknown*: a node potential or a branch flow), discrete pin (on a net of bytes), event pin (on an *event*) | 3 |
| Linkages | private; port; parameter; and the ten *session nets*: temperature, temperature given, resolution, four analysis kinds, start, settle, session parameter | 13 |
| Analog tables | term, limit, small-signal source (a noise source or a stimulus source), crossing, operating-point variable, trace | 6 |
| Aspects | 16 facets of the eleven tables; the 7 values of the *call environment*; the *step bound* | 24 |

Every other word of the document is an entry of one catalogue, a field of one entry, or a word of the
modelling language used with the language's meaning (a *commit*, a *where condition*, a *hold*).

This arrangement reduces what a reader must hold to be oriented, from about thirty nouns to nine. It
does not reduce what must be learned to master the contract: the nouns are all still there, and the
next section counts them.

## 0.2 The counts

Counted by the hostile method of the review's critic: every defined noun, every struct, every constant,
every field a tag governs, every case of every rule. "Before" is the proposal's fourth version as its
critic counted it, given so that the trend can be seen; the third version is the one the user rejected.

| What is counted | Today | v3 (rejected) | v4 | v6 |
| --- | --- | --- | --- | --- |
| Kinds a reader must hold to be oriented | 13 (the concepts of abi.md) | 26 | 19 | **9** |
| Defined nouns in all | about 25 | 26 | about 30 | **37** by the widest count; **30** by the method applied to the other columns |
| Tables of a shape | 6 | 18 | 10 | **11** |
| Structs of the binary contract, primitives included | 26 | — | 31 | **32** |
| Entry points, device side + solver side | 5 + 3 | 6 + 2 | 5 + 2 | **5 + 2** |
| Named groups of constants; constants | 15; 76 | 14 + 5; — | 14; 73 | **11; 66** |
| Presence bits; named values | mixed with flags | 5 groups | 9; 2 | **5 bits in 2 structs; 3** |
| Aspects | 28 legal addresses | 28 | 24 | **24** |
| Tags | 7 | 3 | 7 | **3 stored, 1 derived** (the kind of a function) |
| Fields or items that a tag governs | 6 | 0 claimed | at least 8 | **8** |
| Validation rules; cases | 23 | 18 | 10; about 37 | **9; 22**, and one access table of 13 rows with 3 conditions |
| Checks at bind and at `create` | — | — | 6 | **7** |
| Refusals of things accepted today | — | 5 claimed | at least 9 | **6**, listed whole in section 6 |

The first row compares presentations, not designs: v3 and v4 had no second level, so their numbers are
their flat lists of concepts.

What the numbers say, plainly.

- **The vocabulary is at its floor.** Fifteen of the nouns are the language's or the field's own (port,
  parameter, event, limit, crossing, noise source, trace and so on), and the ABI must give each a
  place. Of those the ABI adds, seven are an adjective on another (three sorts of pin, two kinds of
  function, the default shape, the session nets), and two (*net*, *unknown*) are what a pin is defined
  by. The proposer and the critic each looked for a design with fewer things and found none that does
  not bring back a tag deciding which fields an entry has, which is the defect of the present ABI
  (section 6). What can be reduced is how many must be held at once, and that is section 0.1.
- **Tables and structs are above today's**, because each present table that covered entries with
  different contracts was split (section 0.3).
- **Everything a reader has to check a shape against fell**: three stored tags where there are seven,
  66 constants where there are 76, 22 cases of nine rules where there are 23 rules.

The eight governed items are: `Linkage.port_position` (ports only); `Term.state_tolerance` and
`Term.maximum_delay` (one operator each); a pin's `name`, which is data under two linkage kinds; the one
flag of `Linkage.flags`, which has an effect only where a net can be missing; the second clause of
`STRUCTURAL`, which speaks of parameters; the reading of `initial_value` as a parameter's default; and
the length of a strength, 8 or 16 bytes by the kind of source.

The 37 nouns are 8 of the core (library, device type, instance, shape, layout, pin, linkage, aspect);
22 of the catalogues (3 pins; port, parameter, session net, session parameter; the 6
analog tables with noise source and stimulus source; function with its two kinds, reaction, commit
site; call environment, step bound); and *default shape*, *net*, *unknown*, *event*, *commit*, *where
condition*, *hold*.

## 0.3 The two organising ideas

**Idea 1. The solver keeps and the code computes; a shape lists where values are kept and what
computes, and nothing else.** A device has no memory, so every value that outlives a call is kept by
the solver. The shape has three groups of tables and needs no fourth: **pins** (three tables), **the
analog tables** (six) and **code** (two).

The asymmetry between the two sides follows from one fact. Digital code produces its results *in* pins:
a digital function writes a discrete pin, and a reaction commits to one. Analog code cannot do that,
because the net of a continuous pin is an unknown, which the solver solves and nobody assigns.
Everything analog code produces needs another place to go, and the six analog tables are those places.
That is also all the six have in common, and the proposal claims no more for the group. The same fact
gives the two kinds of function: a function that writes pins is digital, and one that writes analog
tables is analog.

All code names what it touches in one way: an **aspect** and an index. Reading an aspect gives the
facet's state; writing it sets that state.

**Idea 2. Every table has a one-sentence contract.** The contract says what the solver MUST do for an
entry of the table. It is normative in Part II; here are the twelve.

| Table | The contract |
| --- | --- |
| `continuous_pins` | Solve the pin's unknown, and give code its value. |
| `discrete_pins` | Hold the net's bytes; change them only at a microstep; say what changed. |
| `event_pins` | Mark the microsteps in which the net fires, and run the reactions it triggers. |
| (the field `linkage` of those three) | Put the pin on the net the linkage names. |
| `terms` | Add the value, through the term's operator, to the equation of one unknown and subtract it from another's. |
| `limits` | Hold an iterate between Newton iterations, and do not converge while it differs from the device's proposal. |
| `small_signal_sources` | Read as zero in every solve; in the source's small-signal analysis, inject the strength through the declared partials. |
| `crossings` | When the value goes from non-positive to positive between accepted solutions, locate the instant and fire an event. |
| `operating_point_variables` | At each accepted solution, store the value for the host to read by name. |
| `traces` | Deliver every record that an evaluation wrote, whatever becomes of the step. |
| `functions` | Call the code when its kind's schedule requires, and never otherwise. |
| `reactions` | Call the code when an event that triggers it fires, and queue its commits. |

Entries with different contracts are in different tables. That is why a crossing, an operating-point
variable and a trace are three tables although each is "a value that enters no equation", and why a
limit is apart from a small-signal source.

The converse is not a rule, and the proposal does not claim one. In three places entries whose
contracts differ in more than a word share a table, by judgement, and here is the judgement.

| Merged | What differs under the one sentence | Why merged all the same |
| --- | --- | --- |
| the three term operators | what the solver keeps (nothing; a history of values and currents; a history of values) and what can follow (nothing; a rejected step; a failed solve) | Every term ends in the same two equations, and the three share every field but one tolerance each and every aspect but `TERM_DELAY`. Split, they are three tables for one idea that every circuit simulator calls a contribution. |
| analog and digital functions | the caller, the schedule, the partials wanted, the statuses, the error report (14.2) | They share the struct, the frame, the entry point and the purity contract, and they are one table today. Split, a second entry point returns and a function index needs a table beside it. |
| the thirteen linkages | a port has a position, windows and absence; a parameter has host writes and the shadow; a session net is set by the owner | One question, "which net?", with one answer per pin. Split, they are three tables that can name a pin twice or not at all. |

---

# Part I. The proposal

## 1. The problem

### 1.1 What the device ABI is

Piperine simulates mixed-signal circuits. Its solver is three crates: `piperine-solver`, called the
**owner**, which runs the analyses and coordinates the other two; `piperine-analog-solver`, which
solves the continuous equations; and `piperine-digital-solver`, which holds discrete values and runs
events. A **device** is a model of a component (a resistor, a flip-flop, an emulated processor). The
**device ABI** is the binary contract between the solver and the code of a device, and it is the crate
`piperine-abi`.

The central idea of the present ABI is sound and is kept whole: *a device is immutable data plus pure
code, and it has no memory*. The data, called the **shape**, describes everything an instance has. The
code computes outputs from inputs and nothing else. Every byte that exists over time (the unknowns of
the circuit, the discrete values, the pending writes, the integration histories) belongs to the solver.
Because of that the solver can undo a step, redo it, run the digital side ahead of the analog side and
evaluate devices in parallel, with no cooperation from any device.

### 1.2 The user's complaint

The user's words, translated: *"What unsettles me most about the solver is the lack of separation of
some concepts… the names do not match what things do, I could not tell where one thing goes and where
another goes… It is confusing, and I do not see this much confusion even in the Java specification."*

The Java Virtual Machine Specification is the benchmark, and it is clear for three reasons that can be
stated. Every datum has exactly one place (a run-time data area, or a table of the class file). Every
table holds one sort of thing. Every name is used for one thing throughout the text. The diagnosis
below measures the present ABI against those three properties.

### 1.3 The present vocabulary, in one paragraph

To follow the diagnosis the reader needs today's words. A shape has six tables. A **pin** is a net seen
from an instance; its `kind` says what the net holds (`CONTINUOUS`, `DISCRETE`, `EVENT`) and its `role`
says who binds it (`PORT`, `PARAM`, `GLOBAL`, `PRIVATE`). A **term** is a value added to an equation. An
**input** is a value the solver supplies that is not a net: a *held* input (`LIMIT`, `NOISE`, `STIM`) or
an *ambient* one (the time, the frequency). A **watch** is an analog output that enters no equation
(`VAR`, `CROSS`, `CAP`, `TRACE`). A **function** is pure code with declared reads and writes. A
**reaction** is code run when an event fires; it writes through **statements**, by calling **commit**.
Everything a function reads or writes is addressed by a **cell**, the triple (`kind`, `index`, `part`).
A **module** is a named factory of instances with a **prototype**, the shape at default parameter
values.

### 1.4 Diagnosis

Each finding is classified by how it confuses: a **homonym** is one word for two things; a **synonym**
is two words for one thing; an **opaque** name does not say what the thing does; a **misplaced**
responsibility sits in the wrong table or the wrong layer.

**D1. `Pin` is nine things in one struct (misplaced, opaque).** `Pin` (`abi/raw.rs:490`) has twelve
fields, and six apply only to some pairs of `kind` and `role`; nine of the twelve pairs are legal
(`abi/validate.rs:367`–`:374`). The struct answers two unrelated questions at once, what the solver
stores and how it is connected. The field `init`
means three things depending on kind and role (`abi/raw.rs:505`). The seven flags of `pin_flag`
(`abi/raw.rs:786`) mix structure, binding, presence of another field, change tracking, observation and
scheduling.

**D2. The kind of a function is defined nowhere and deduced three times (misplaced).** One table and
one entry point, `eval`, serve analog functions and discrete functions. Which one a function is, is
deduced from what it writes by three predicates: in the analog solver (`an/plan.rs:236`), in the digital
solver (`dg/plan.rs:269`) and in the validator (`abi/validate.rs:547`). The two kinds differ in caller,
schedule, legal reads and writes, and legal statuses. The proposal gives the kind one definition; it
does not stop the kind from being read off the writes (section 6).

**D3. Statement, commit, pending, writes (synonym, homonym).** A reaction's list `writes` holds
`Statement`s; a function's list `writes` holds `Cell`s (`abi/raw.rs:606`, `:622`). The declared thing is
a *statement*, its use is a *commit*, and in the queue it is a `Pending` (`dg/queue.rs:22`).

**D4. "Input" has four meanings (homonym).** A held input, an ambient input, the frame array
`Eval.input` that holds every read whatever it reads (`abi/raw.rs:654`), and `ShapeInput` in the digital
plan (`dg/plan.rs:55`). A held "input" is mostly written: `PROPOSED`, `LIMITED`, `PSD`, `RE` and `IM`
are outputs of functions. Within a limit, the written `LIMITED` and the read `VALUE` are one number
(`an/assembly.rs:44`–`:45`).

**D5. "Watch" groups four outputs by what they are not (opaque).** The four kinds have four final
users: the host, the event queue, the step controller, the observation sink. `CROSS` is the one
mechanism by which the analog side produces events, and it is filed among observations.

**D6. A cell's part changes meaning with its kind (homonym).** The part `TIME` on a term is a delay; the
ambient constant `TIME` is the time. The part `CHANGED` exists on two kinds of pin and can be written on
one of them only. For an ambient cell, `index` is a constant, not an index. Array access is a parallel
list, `elements`. Which part is legal for which kind is a table of fifteen rows (manual §5.2).

**D7. Shape, prototype, module, device, factory (synonym, misplaced).** `Module` is "a named factory"
(`abi/raw.rs:358`); the Rust traits are `Device` and `Factory` (`abi/device.rs:16`, `:31`); all three
name the type of a device. The prototype is a shape, an instance-level structure, used as the
type-level table of parameters, and rule P5 of abi.md ("`PARAM` pins come first in every shape")
exists only so that an index into one shape means the same in another (`abi/validate.rs:43`).

**D8. "Host" names two things (homonym).** The host is the program that drives the simulation
(`sv/protocol/host.rs`). `Host` in the ABI is the table of functions a reaction calls to commit
(`abi/raw.rs:682`), which is the digital solver.

**D9. Smaller homonyms.** Four unrelated fields named `kind`. "Seed" for the wanted partials, for
seeding a parameter net and for a limiter's starting value. `Flow` (the result of a reaction,
`abi/device.rs:46`) and `FLOW` (a term operator). Two structs named `Ambient` (`an/assembly.rs:28`,
`dg/run.rs:25`) and two named `Accepted`. "Discrete" in the ABI and in the trait `DiscreteSide`,
"digital" in the crate and its struct `Digital`. `FLOW` for the row of a voltage source, which is not a
flow.

**D10. Responsibilities in the wrong layer.** `RESHAPE`, which means "these parameter values need
another shape", is a status of the general `eval` entry although only some discrete functions may
return it, and only in one context (`dg/run.rs:328`). The frame fields `Shape.size`, `Eval.size` and
`React.size` are read by no consumer (manual §8.3 item 5). The seam between the owner and the
sub-solvers passes nets, slots, events and samples as bare `u32` (`sv/protocol/sides.rs`). The module
`piperine_solver::abi` exports the circuit builder, not the ABI.

**D11. The ABI ignores the words of the language above it.** The language specification defines, in
the meaning the ABI needs, *commit* (`spec/part_iv_behavior.md:374`), *operating-point variable*
(`:126`), *noise source*, *trace*, *relaxation*, *equilibrium solve* (`spec/part_v_execution.md:70`) and
two classes of *environment* (`spec/part_iv_behavior.md:744`–`:745`). The ABI says `VAR`, `STATIC`,
`RELAX`, `ambient` and `GLOBAL` for them, and uses the language's word *check* (a record on the
`checks` stream, `spec/part_vi_io.md:95`) for a `where` condition (`plan/plan.rs:79`).

**D12. A comment and its consumer disagree.** `abi/raw.rs:509` documents `Pin.abstol` as "tolerance of
the unknown, of its row". The plan fills it with the nature's potential and flow tolerances
(`plan/plan.rs:622`) and bind applies the first to a node and the second to a branch unknown
(`sv/core/bind.rs:656`–`:662`), as the language requires (`spec/part_v_execution.md`, §17.1).

**D13. Net properties are declared per pin and folded silently.** `QUIET`, `RECORD`, `RELAX` and
`STRUCTURAL` are flags of a pin and properties of its net: bind ORs them over every pin on the net
(`sv/core/bind.rs:495`–`:498`). No document says so.

**D14. Nothing guards an array read from analog code.** The elaborator guarantees that analog code never
reads an array: it makes a scalar mirror of each element the `analog` block reads
(`crates/piperine-lang/src/elab/elements.rs:220`–`:282`). But neither the validator
(`abi/validate.rs:447`) nor the plan (`plan/plan.rs:312`–`:314`) refuses such a read, and the analog
solver would copy a whole slot whatever the read's length (`an/assembly.rs:678`). A foreign producer has
no guard.

**What is not wrong, and is kept:** the device has no memory; a term has a linear time operator at the
top of an equation; events are the only trigger of reactions; one writer per net; a value is its bytes;
reads, writes and partials are declared, so that the solver plans everything at bind. The words
*instance*, *shape*, *layout*, *term*, *partial*, *reaction*, *event*, *tick*, *microstep*, *instant*,
*time point*, *landing* and *horizon* each name one thing and are kept.

## 2. Design principles

The proposal follows six rules. Each later choice can be checked against them.

**Rule 1. Every table has a one-sentence contract, and entries with different contracts are in
different tables.** Section 0.3 gives the twelve contracts and the three places where, by stated
judgement, one table holds entries whose contracts differ in more than a word. A tag MUST NOT change
the meaning of a field. Where a tag governs whether a field applies, the proposal counts it (section
0.2).

**Rule 2. A word of the language specification means what it means there; any other word is owned by
nothing else in the tree.** Names are whole words. The names of analyses (`DC`, `AC`) are proper names
and are kept, as in the project's own `Analysis::Dc`.

**Rule 3. What a net holds and which net it is are two questions with two places.** The table of a
pin says what its net holds. The field `linkage` says which net it is. Two linkage kinds also give a
job to the pin's name, and section 12 says so.

**Rule 4. The kind of code is defined once.** A function is digital when it writes pins and analog
when it writes analog tables. The definition is in the specification and is implemented once in
`piperine-abi`; no consumer has a predicate of its own. The kind is still read from the writes and not
declared; Part IV, decision 4, puts that choice to the user.

**Rule 5. Absence has one encoding per type of field.** A field that holds an index is absent by a
named value that no index can equal (`GROUND`, `SESSION_DEFAULT`). A field that holds a value is absent
by a clear bit of `present`. A descriptive field that says nothing is empty.

**Rule 6. What is unchanged is said to be unchanged.** The proposal moves and renames. Section 6 lists,
whole, the six things that are accepted today and refused by it, and the fields that are ignored today
and can no longer be written.

### 2.1 The rule for two pairs of words

The tree uses *analog* and *continuous* for one side, and *digital* and *discrete* for the other, with
no rule. The language specification has one, and the proposal adopts it under rule 2:

> ***Continuous* and *discrete* describe what is held. *Analog* and *digital* describe code, and the
> solver that runs it.**

The language says *continuous nature*, *discrete nature* and *discrete carrier* for what holds a value
(`spec/part_ii_carriers.md:185`, `:392`), and `analog` block and `digital` block for code
(`spec/part_iv_behavior.md`, terms). Applied here: the pins are the **continuous pin** and the
**discrete pin**; the functions are the **analog function** and the **digital function**; the six
tables that only analog functions write are the **analog tables**; the crates stay
`piperine-analog-solver` and `piperine-digital-solver`, which are the user's own words (decision
MD-60). Inside a solver crate, a type is named for the solver that owns it: `AnalogVariable`
(`crates/piperine-analog-solver/src/netlist.rs:77`, 113 uses) is the analog solver's own unknown, a node
or a branch, and a continuous pin is bound to one. The only identifiers that break the rule today are
on the seam: the trait `DiscreteSide` becomes `DigitalSide`, and the adapter that has that name today
is renamed (section 7, step E). This is decision 1 of Part IV.

## 3. The concepts

Section 0.1 gave the nine kinds of the core. This section defines each entry of the four catalogues in
one sentence; Part II specifies them.

**Packaging.** A **library** is a versioned collection of device types exported under the symbol
`piperine_abi_1`. A **device type** is a named kind of device: its four entry points and its *default
shape*, the shape of an instance created with no parameter value given, from which a host and a loader
learn about the device type without creating anything. An **instance** is one occurrence of a device
type in a circuit. A **shape** is the complete, immutable description of the structure of one instance.
A **layout** is a size, an alignment and fields over a closed set of scalars.

**The catalogue of pins.** A *net* is something of the circuit that instances share: an *unknown* of
the analog solver (a node potential or a branch flow), a store of bytes in the digital solver, or an
*event*. A **pin** is the instance's end of one net.

| Pin | Definition |
| --- | --- |
| **Continuous pin** | A pin on an unknown: one real number that the analog solver determines. |
| **Discrete pin** | A pin on a net that holds bytes of a layout, which change only at a microstep; one value, or an array of them. |
| **Event pin** | A pin on an event: a net that holds nothing and fires at a microstep. |

**The catalogue of linkages.** The **linkage** of a pin names its net.

| Linkage | The net |
| --- | --- |
| private | the instance's own; nothing else reaches it |
| **port** | the one the netlist connects, by position |
| **parameter** | one that the host writes under the pin's name |
| ten **session nets** | one that the session provides for the whole design: the temperature, whether it was given, the time resolution, the four kinds of analysis, the events `START` and `SETTLE`, and a named *session parameter* |

**The catalogue of analog tables.**

| Analog table | Definition |
| --- | --- |
| **Term** | A real value added to the equation of one unknown and subtracted from that of another: directly, through a time derivative, or after a delay. |
| **Limit** | An iterate that the solver holds between Newton iterations for one nonlinear quantity. |
| **Small-signal source** | A source that is zero in every solution and that one small-signal analysis injects: a noise analysis (a *noise source*) or an AC analysis (a *stimulus source*). |
| **Crossing** | A real value whose passage from non-positive to positive fires an event, at a time the solver locates. |
| **Operating-point variable** | A named value that the solver stores at each accepted solution and the host reads. |
| **Trace** | A record that an evaluation writes and the host receives whatever becomes of the step. |

**Code.** A **function** is pure code with declared reads, writes and partial derivatives. One that
writes analog tables is an *analog function*, called by the analog solver when a solve needs a result
of it; one that writes pins is a *digital function*, called by the digital solver when something it
reads changed. A **reaction** is code called at a microstep in which an event that triggers it fired;
its only effect is *commits*, made through its **commit sites**. A commit is a write of bytes to a
discrete pin, or a firing of an event pin, that applies at a later instant.

**The catalogue of aspects.** An **aspect** is one facet of the entries of one table; with an index, it
is how code names anything it reads or writes. Sixteen aspects belong to the tables (section 14.1 lists
them). Eight belong to no table: the seven values of the **call environment**, which a call receives
(the time, the frequency, the two continuation values, whether the solve is an equilibrium, the tick
and the horizon), and the **step bound**, a result that caps the next transient step and needs no entry
to hold it.

A **where condition** is a boolean discrete pin, marked by a flag, that must hold after any host write
to a parameter or to a session net. It is the language's word and needs no table.

### 3.1 Where does X go?

One place for every sort of datum and every operation a device needs. The left column is what an author
has in hand; the right is the one place it goes.

| What the author has | Where it goes |
| --- | --- |
| a port | a pin whose linkage is `PORT` |
| an internal node; the flow unknown of a potential branch | a continuous pin whose linkage is `PRIVATE` |
| the initial condition of a node (`= value`) | `hold_potential` of its continuous pin |
| the absolute tolerances of a nature | `potential_tolerance` and `flow_tolerance` of the continuous pin |
| a parameter; its default | a discrete pin whose linkage is `PARAMETER`; its `initial_value` |
| a parameter whose every change needs another structure | the same pin, flagged `STRUCTURAL` |
| a parameter value that this structure cannot take | the status `RESHAPE`, from a digital function of the parameter cone |
| a rule that a parameter must obey (`where`) | a digital function that computes a boolean discrete pin flagged `WHERE_CONDITION` |
| the temperature, a session parameter, the kind of analysis | a discrete pin linked to that session net |
| the time, the frequency, `gmin`, the source scale, the tick, the horizon | a read of that aspect of the call environment |
| setup code; what depends on the temperature or on parameters only | a digital function that reads those pins and writes a private discrete pin |
| device memory; a piece of state; a register | a private discrete pin, written by commits |
| an array or a memory | a discrete pin with an `element_count` |
| a timer; a breakpoint; "wake me at" | a commit with a delay to a private event pin |
| a current; a charge; a delayed value | a term with operator `DIRECT`; `TIME_DERIVATIVE`; `DELAY` |
| the tolerance of a `ddt`; the bound of a `delay`; the tolerances of a `cross` | a discrete pin holding a real, computed by a digital function and named by the term or the crossing |
| junction limiting | a limit |
| `noise(…)`; `ac_stim(…)` | a small-signal source of kind `NOISE`; `STIMULUS` |
| `cross(…)` | a crossing, and the event pin it fires |
| `bound_step(dt)` | a write of the aspect `STEP_BOUND` |
| a named `let` of an `analog` block | an operating-point variable |
| `emit!` in an `analog` block | a trace |
| `emit!` in a reaction; a record stream | a commit to a discrete pin flagged `RECORD_STREAM` |
| the Jacobian | the partials of the functions |
| `load` for DC, AC and transient; the noise function; the accept hook | functions; which solve calls one follows from what it writes |
| a failure; `finish()` | the status `FAULT` with a `FailureMessage`; the status `FINISH` from a reaction |
| a checkpoint, a restore, a capability flag | nothing: the device has no memory |

### 3.2 The cross-layer glossary

Every noun against the language specification and the identifiers of the tree. No row reads "same
word, different thing".

| ABI noun | In the language specification | Relation | In the tree |
| --- | --- | --- | --- |
| pin | not a term of the language (the word occurs twice, in prose) | the ABI's own word, kept from the present contract: "a pin is a net seen from an instance" (`abi/raw.rs:487`) | `pin` in about 100 source files, with this meaning |
| net | used in prose for what carriers are connected to; not a defined term | the solvers' word: what instances share | `NetId`, `NetKind`, `NetSpec` (`dg/net.rs`) for discrete and event nets; an unknown is the analog solver's counterpart |
| unknown | "the analog unknowns", "branch unknown" (`spec/part_v_execution.md:63`, `:607`) | same | `Site::Unknown` (`sv/core/bind.rs:86`), `AnalogVariable` |
| continuous pin | a *node* is a carrier of a continuous nature | wider on purpose: also a branch flow and a hoisted operator | on the analog solver's `AnalogVariable`, which the tree also calls the *unknown* (`sv/core/bind.rs:86`) |
| discrete pin | a *discrete carrier*, or a *param* | wider on purpose: also compiler-made state, where conditions and tolerances | `NetKind::Discrete` (`dg/net.rs:11`) |
| event pin, event | Part IV | same | `NetKind::Event` |
| linkage | none | a new word: what the shape *declares* about a pin's net. *Binding* was not chosen because the tree owns it for what bind *made* of the declaration (`dg/plan.rs:20`, `an/plan.rs:22`), and the language owns it for a pattern binding | no such identifier in `docs/spec` or `crates/*/src` |
| port | *port*: a carrier, a bundle of carriers or a value input | one pin per carrier of a language port | the solver's `Port` (`sv/core/bind.rs:33`) is the other end: what the netlist hands to the pin |
| parameter | *param* (`spec/part_iii_structure.md:101`) | the keyword in full | `Param` |
| session parameter | `simparam(name)` (`spec/part_iv_behavior.md:744`) | same | "session parameter" (`plan/plan.rs:962`) |
| session net | "environment, constant during an analysis" (`:744`); a *session* is the host's handle (`spec/part_viii_host.md:20`) | same class | replaces `GLOBAL` |
| structural | *structural param* | same | `plan/plan.rs:603` |
| where condition | "`where` condition" (`spec/part_iii_structure.md:122`, §8.1.23) | same | replaces the ABI's `check` |
| hold | "tied to that potential by a potential source to `gnd`, the **hold**" (`spec/part_v_execution.md:428`–`:430`) | same | `Hold` (`sv/core/bind.rs:648`) |
| initial value | "A carrier, a drive and a param start with a value. The value is the initial value", committed at instant (0, 0) (`spec/part_i_values.md:1204`, `:1251`) | same: the bytes a discrete net starts with, which for a parameter is its default | `Pin.init` |
| term | a *contribution* is a source statement | different word, different thing: a potential contribution lowers to two terms | — |
| limit, crossing | `limit`, `cross` | same | — |
| noise source | "A noise source is an instance of `noise`" (`spec/part_iv_behavior.md:306`) | same | `NoiseSource` (`crates/piperine-ir/src/model.rs:438`), same thing |
| stimulus source | `ac_stim`, "small-signal stimulus" (`spec/part_iv_behavior.md:743`) | same | `stims` in the IR |
| small-signal source | "small-signal" as in *small-signal rule* and *small-signal stimulus* | the collective of the two rows above | `SmallSignal` (`an/small_signal.rs:113`) |
| operating-point variable | a named `let` of an `analog` block (`spec/part_iv_behavior.md:126`) | same word, same thing. The ABI uses *variable* for nothing else. | — |
| trace; record stream | Part VI (`spec/part_vi_io.md:67`) | same | `Trace` in the test crate `piperine-parity` is a waveform; noted, left alone |
| step bound | `bound_step(dt)` (`spec/part_iv_behavior.md:1026`) | same | — |
| commit | `q <- x;`, a write that applies at a later instant (`spec/part_iv_behavior.md:374`) | same word, same thing | `Commit` in several crates |
| commit site | *site*: the place of an `emit!` | same sense of "site" | `Record { site }` (`sv/protocol/sides.rs:217`). The owner's `Site` enum (`sv/core/bind.rs:84`) is renamed in step E. |
| reaction | Part IV | same | — |
| analog function, digital function | `analog` block, `digital` block | the same adjectives; "function" because one may hold many source items | — |
| equilibrium | *equilibrium solve* | same | replaces `STATIC` |
| call environment | "environment of the solve" (`spec/part_iv_behavior.md:745`) | the language's class, extended: the environment of an instant is not a solve's, and adds the tick and the horizon | replaces `ambient` |
| device type | *module* | different word on purpose: the language owns *module* | — |

Words the proposal does not use because the language or the tree owns them with another meaning:
*variable* (a `let` binding, `spec/part_i_values.md:695`; kept only inside *operating-point variable*),
*carrier*, *signal*, *check*, *drive*, *quantity*, *state*, *module*, *output*, *binding*, *probe*,
*measurement* (a host-side computation over results, `sv/analyses/tf.rs:6`,
`spec/appendix_g_scope.md:219`), *register* (a user's own module, `spec/appendix_e_gallery_digital.md:23`).

## 4. Run-time data areas

The sentence "the device has no memory" can only be checked against a complete list of where memory
is. This is that list. Every area exists in the code today; the proposal names each once. For a device
author three columns matter: who writes the area, whether a restore puts it back, and whether its
declaration is per instance or folded over every pin on a net.

| Area | Holds | Owner | Declared by | Written by | Restored |
| --- | --- | --- | --- | --- | --- |
| Solution vector | one real per continuous pin | analog solver | `continuous_pins`; a node's tolerance is the smallest over the instances on it (`sv/core/bind.rs:656`–`:658`) | the solver | yes |
| Discrete store | per discrete pin: bytes, previous bytes, last change; sparse for arrays (`dg/store.rs:125`) | digital solver | `discrete_pins`; the flags `UNTRACKED`, `RECORD_STREAM`, `RELAXED_LOOP_ALLOWED` and `STRUCTURAL` and the initial value are folded over the pins on the net (`sv/core/bind.rs:495`–`:505`) | applied commits, digital functions, the host | yes |
| Parameter nets | the store entries behind the parameters, one per instance or one shared by a model card (`sv/core/bind.rs:709`–`:722`) | digital solver | a linkage of kind `PARAMETER` | the host only | yes |
| Session nets | temperature, analysis kind, resolution, named session parameters, `START`, `SETTLE` | digital solver | a linkage to a session net | the owner and the host | yes |
| Host initial writes | the host's accepted writes between analyses, replayed at the start of each (`sv/core/circuit.rs:249`) | owner | — | the host | not part of a step |
| Shadow store | a copy of the discrete store on which a host write is tried (`dg/engine.rs:540`) | digital solver | — | the parameter cone | discarded |
| Event marks | whether each event fired in the present microstep | digital solver | `event_pins` | commits, digital functions, crossings | yes |
| Commit queue | pending commits, each with the instant it was made and the instant it is due (`dg/queue.rs:22`) | digital solver | `commit_sites` | `commit`, the host, the owner for crossings | yes |
| Held settle commits | commits of a settle phase, due at the next time point (`dg/settle.rs:23`) | digital solver | the same | `commit` in a settle phase | yes |
| Analog copy | the bytes and previous bytes of every discrete pin an analog function reads (`an/assembly.rs:168`) | analog solver | reads of analog functions | the owner's delivery | yes |
| Term histories | for time-derivative terms, past values and currents; for delay terms, past values (`an/ddt.rs:59`, `an/delay.rs:20`) | analog solver | `terms` | acceptance of a solution | yes |
| Limit memory | held iterate, proposed value, previous iterate, age (`an/assembly.rs:43`) | analog solver | `limits` | each iteration; acceptance | yes |
| Accepted values | per crossing, its value at the last accepted solution and the locator's bracket; per operating-point variable, its bytes at the last accepted solution (`an/watch.rs:42`–`:44`) | analog solver | `crossings`, `operating_point_variables` | acceptance | yes |
| Function results | the last read bytes, writes and partials of each analog function (`an/assembly.rs:57`). It is the memo, and it is also where the solver reads term values and the strengths of small-signal sources from (`an/small_signal.rs:60`–`:62`, `an/plan.rs:471`) | analog solver | the analog `functions` | each call | no: recomputed from the restored reads, because a function is pure |
| Observation buffer | observations of the digital side since the last accepted step (`sv/protocol/sink.rs:19`) | owner | — | the digital solver | truncated |
| Trace records | the records that evaluations wrote and no acceptance has reached yet (`sv/protocol/sink.rs:22`) | owner | `traces` | every evaluation that writes one | **no: a restore keeps them** |
| Passing values | the first value each analog-read net lost within the instant (`dg/passing.rs:15`) | digital solver | — | the microstep loop | within the instant |
| Protocol statistics | what ended each step (`sv/protocol/statistics.rs:11`) | owner | — | the protocol | no |
| **Instance** | nothing observable | the device | — | the device | no, and it must not need to be |

The tolerances of time-derivative terms, the bounds of delay terms and the tolerances of crossings are
not an area of their own. Each is a discrete pin of the store that a digital function computes
from parameters (`plan/plan.rs:90`–`:99`).

A device type whose instances keep observable state outside these areas (a co-simulated foreign
process) marks the reactions that touch it `IRREVERSIBLE` and pays the price described in section 5.

## 5. How each phase of the solver uses the ABI

Nothing in this section is new behaviour. It restates, in the proposed words, what the solver does
today, so that the reader can check that each phase finds what it needs.

**Load.** The loader calls `piperine_abi_1`, checks `Library.size` and `Library.minor`, and reads the
device types. For a compiled design, the transpiler compares the default shape of every device type
with the shape its own plan computed, entry by entry, and refuses the library on the first difference
(`tp/compiled.rs:66`–`:71`). A host reads the parameters of a default shape to learn each one's name,
layout and initial value, and encodes the values it was given.

**Create and bind.** The owner calls `create` once per instance. Bind validates the shape and reads it
by group. From the *linkage* of each pin it finds the pin's net: a port's by its position, a
parameter's by name, seeded with the given value or the pin's initial value, a session net's by its
kind; a private pin gets a net of its own. From the *analog tables* the analog solver resolves every
term to two equations and builds the sparsity pattern from the partials of the analog functions,
directly and through the partials of a limit's proposed value; the pattern depends on declarations
only and is fixed here. Each analog function is classified, by the aspects it writes, into the solves
that need it. From the digital functions and the reactions the digital solver derives the one writer
of every net, orders the digital functions, computes the **parameter cone** (the digital functions
whose inputs derive only from parameters and session nets), and ignores every minimum delay under one
tick.

**A DC solve.** Each Newton iteration calls `evaluate` on the analog functions in two passes per instance: first the
functions that write a limit, then those that write terms. A function is called only when the bytes it
reads differ from those of its last call. Reads come from the solution vector, the analog copy, the
limit memory and the call environment (`EQUILIBRIUM` is 1). A term with operator `DIRECT` is stamped in
Norton form with the limit correction; one with `TIME_DERIVATIVE` contributes nothing; one with `DELAY`
contributes its value. The solve has not converged while a limit's value and its proposed value differ
beyond the limiting tolerance. Under homotopy the device sees only `MINIMUM_CONDUCTANCE` and
`SOURCE_SCALE` move. At the accepted solution the solver evaluates the functions that write
operating-point variables, crossings or step bounds. A trace is recorded whenever a function that
writes one is called, and never causes a call. The same evaluation serves the analyses built on
the linearized operating point: transfer function, pole-zero, S-parameters, adjoint sensitivity and
periodic steady state (`sv/analyses/tf.rs`, `pz.rs`, `sp.rs`, `adjoint.rs`, `pss.rs`).

**The mixed operating point.** The digital solver starts: it writes every initial value, fires `START`
and runs the digital functions once. The analog solver solves the equilibrium; while it does, every
continuous pin that has a hold is tied to its hold potential. Crossings that fired are committed
to their events; the digital solver runs the instant; changed discrete pins that analog functions
read are delivered, and the round repeats, at most 20 times.

**A transient step.** The owner takes a checkpoint of both sub-solvers and of the observation buffer.
The analog solver proposes the end of the step (step control, step bounds, the stop time, the next due
commit). The digital solver runs the instants due before it, and stops before a **landing**: an instant
that changes a discrete pin an analog function reads, or fires a reaction that reads a continuous
pin, or fires an `IRREVERSIBLE` reaction or a host waiter. The analog solver attempts the step on
the values from before the instant. A rejection restores the checkpoint. On acceptance the instant
runs, its changes are delivered with a restart mode, and the observations are flushed. The device is
called by the two sub-solvers only, and is never told of a checkpoint, a rejection or a restore.

**Crossings.** After an attempt converges, the analog solver evaluates the functions that write
crossing values at the attempted point and compares each with its crossing reference. A crossing that
went from non-positive to positive is located by secant with the Illinois rule, within the crossing's
time and value tolerances; an unlocated one shortens the step. At acceptance its event is committed at
microstep 0 of the accepted tick.

**Horizon and irreversible reactions.** A reaction that reads the aspect `HORIZON` receives the last
tick through which nothing that triggers it or that it reads can change. When another reaction's commit
comes due earlier, the digital solver lowers the horizon and calls the reaction again; only the last
call's commits stand. An `IRREVERSIBLE` reaction is never run ahead and receives an exact horizon.

**The zero-width solve and the retake.** When a discrete pin that an analog function reads held a
value at the end of one microstep and lost it later in the same instant, the owner delivers those
passing values and asks for a solve of zero width at the instant's time. When the step that arrived
read a jump and a crossing it produced changed what analog functions read, the owner retakes the step.
An instant that ran to its end can be reopened at its next or its last microstep for this
(`sv/protocol/sides.rs:239`).

**AC and noise.** No instant runs. With every limit at its proposed value, each term contributes its
partials times the response of its operator (1, jω, e^(−jω·delay)). In AC, a partial of a term against
a stimulus source's value injects the phasor that a function wrote. In noise, each noise source is an
injection column with the density a function wrote; the report is keyed by the source's name. Only
functions that read `FREQUENCY` are called again per frequency.

**Host actions.** A host write to a parameter net or to a session net is first tried on the shadow
store: the parameter cone runs, and then every where condition of every instance is read. If any is
false the write is refused and nothing changes. If a digital function of the cone answers `RESHAPE`,
the write is refused inside a transient; between analyses the owner destroys, re-creates and re-binds
the instances. A write to a `STRUCTURAL` parameter re-creates on every accepted write and is refused
during a run. A host write may address a byte offset inside a net, and may be tried without being made.
A host drive of any other net is an ordinary commit by the host. A host waiter on an event is treated
as an `IRREVERSIBLE` reaction.

**Observation.** One sink receives changes of discrete nets, commits with the reaction and commit site
that made them, crossings, records and traces. What the digital side observed is buffered until the
step is accepted, and a rejection discards it. Trace records are kept through a rejection and delivered
once an acceptance reaches their time (`sv/protocol/sink.rs:1`–`:8`). The host can also read
operating-point variables, term values, the limits still active and the noise of each source by name.

## 6. Costs

**The contract is larger than today's in tables and structs, and smaller in everything else.** Section
0.2 has the counts. Against the present ABI there are 11 tables where there are 6 and 32 structs where
there are 26. Each added table is a place where one present word covered entries with different
contracts: `Pin` was three sorts of net, a held input was a limit or a small-signal source, and a
watch was a crossing, an operating-point variable, a trace or a step bound.

**The vocabulary is at its floor and will not shrink further.** About thirty nouns, five more than
today (section 0.2). Fifteen are the language's own and need a place. Every way to have fewer things
that the proposer or the critic tried puts entries of different contracts under one tag that decides
which fields an entry has: one table of analog results with a kind is the present `Input` and `Watch`
again; an event as a discrete pin of no bytes needs a special case for size zero; one table of pins is
the present `Pin`. The one remaining trade goes the other way (decision 2, third row). What the
proposal offers instead is that a reader need hold only nine kinds to be oriented (section 0.1), and
that each noun has one place (section 3.1).

**Three tags remain**, with eighteen values: thirteen nets a pin can be linked to, three term
operators, two kinds of small-signal source.

**Eight items are governed by a tag** (section 0.2). `Linkage` is a tagged union and is specified as
one.

**Refusals: the complete list.** It was produced by walking every case of every rule of section 17,
and every field of the present structs, against the present validator and bind. Six things are accepted
today and refused by the proposal.

| # | Refused | Today | Evidence that nothing depends on it |
| --- | --- | --- | --- |
| 1 | A read of an array by an analog function. | Nothing guards it; the analog solver would copy a whole slot whatever the read's length (D14). | The elaborator never emits one (`crates/piperine-lang/src/elab/elements.rs:220`–`:282`). |
| 2 | `QUIET`, `RECORD`, `RELAX`, `STRUCTURAL` or an initial value on an **event pin**. An `EventPin` has no flags and no initial value. | The validator accepts them on any pin (`abi/validate.rs:355`–`:405`), bind folds them into an event net (`sv/core/bind.rs:495`–`:505`), and the digital solver acts on them: a loop may pass through an event net flagged `RELAX` (`dg/dag.rs:49`), and a commit to a quiet net does not prolong the instant (`dg/instant.rs:205`). | No producer in the tree's sources sets one: the plan builds every event pin with no flags (`plan/digital.rs:505`, `:516`, `:665`, `:689`). |
| 3 | The strength of a small-signal source written by two functions, in parts or whole. Bind refuses it, as it refuses a term written by two functions. | `RE` and `IM` are two cells, each with its own writer (`an/plan.rs:452`–`:453`); a second writer of one cell silently replaces the first. | Every producer in the tree writes a strength from one function: the plan (`plan/plan.rs:808`) and the hand-written devices of the tests. |
| 4 | A session net linked from the wrong table: `START` or `SETTLE` from a discrete pin, or a value net from an event pin. | The validator accepts the pin (`abi/validate.rs:367`–`:374`); bind then fails to encode the value, or makes a net that nothing fires. | It never worked. |
| 5 | Two parameters of one name in a shape. | Accepted; the second replaces the first in the owner's map, so the host reaches only one (`sv/core/bind.rs:721`). | It never worked. |
| 6 | A nonzero element on a read that is not of a discrete pin. | The parallel list `elements` is looked at only for pin cells (`abi/validate.rs:442`–`:457`). | It never meant anything. |

Beside those, some fields are *ignored* today on the entries where they do not apply, and can no
longer be written there, because the struct that held them is split. No behaviour changes. They are:
`QUIET`, `RECORD`, `RELAX`, `STRUCTURAL` and `abstol` on a pin of a kind that does not read them;
`Pin.global` on a pin whose role is not `GLOBAL` (`abi/validate.rs:361`); the layout of a `CROSS` or
`CAP` watch, and `event`, `ttol` and `etol` of a watch that is not `CROSS`; the unit of an event pin and
the name of a step bound, which are descriptive.

Four things that could have been refused are instead defined, so that no rule is spent on them:
`RECORD_STREAM` alone (it tracks nothing by definition); `OPTIONAL` where a net is always provided (it
changes nothing there, 12.4); `STRUCTURAL` on a discrete pin that is not a parameter (its present
meaning is kept, 11.2); and `OWNS_LIMIT_CORRECTION` on a function that writes no limit (it says nothing
there, 13.2). A tolerance on a term whose operator does not read it is validated and not read, as today
(13.1). One check is looser than today's: the reads of a function that writes nothing are checked for
constants and indices only (14.2), which costs nothing, because neither solver calls such a function.

**One error message is less precise.** `E0509`, a step bound that is not finite and positive, names
the function that wrote it and its origin, where today it names the watch (`an/watch.rs:107`–`:110`).

**The alignment promise is withdrawn.** abi.md §12 promises that frame buffers have the alignment of
their layouts. The analog solver passes one packed byte buffer (`abi/caller.rs:316`–`:322`), and has
never kept the promise. The proposal changes the sentence to what the code does: read pointers carry no
alignment, and a device reads by copy. This is an exception to the project's rule that code follows the
specification, and decision 6 asks for it.

**Names change once.** Contributors relearn part of the vocabulary; abi.md and the manual are
rewritten. *Pin* stays, so the commonest word of the present code does not move.

**The migration writes temporary code** and deletes it (section 7).

**What the proposal does not fix.**

- *The kind of a function is still read from its writes* (finding D2). There is now one definition and
  one implementation of it, where there were two predicates. A reader of a shape must still look at
  what a function writes to know which solver calls it, what it may read and which statuses it may
  return. Decision 4 offers the declared alternative.
- *`RESHAPE` is still a status of the general entry `evaluate`*, legal for some functions in one
  context (finding D10); the rule is stated in one place (16.3) and nothing more is claimed.
- *The six analog tables share only that analog code writes them* (the first half of finding D5). They
  are grouped for that reason and no other.
- *`pin` is broader than the word suggests* (decision 2): an internal node, a parameter and a tolerance
  are pins.

Also unchanged: The analysis is still described in two places (which analysis runs is a session net;
whether this solve is an equilibrium is in the call environment), because the two have different
lifetimes. `force` and `release`, which the specification has and the code does not, are untouched
(Part IV, decision 7). An emulator reachable by a crossing still cannot run ahead; there is still no
text leaf, no gradient across events and no loader for arbitrary libraries; the shadow still copies the
store.

## 7. The migration plan

### 7.1 The idea

The tree holds 180 implementations of the traits `Device` and `Factory`: 15 in the tests of
`piperine-abi`, 29 in those of the analog solver, 30 in those of the digital solver, 96 in
`piperine-solver` (7 of them in its sources, `sv/analyses/test_devices.rs`), 1 in `piperine-interp` and
9 in `piperine-transpile`. A plan that rewrites them more than once, or that needs one commit across
every crate, is not acceptable.

A device author and a generator touch the ABI only through the **authoring surface**: the shape
builder, the traits and the lenses. The solvers touch it only by **reading shapes** and by **calling**.
The raw layout sits between the two. So the authoring surface moves first, the reading side second,
and the raw layout last.

### 7.2 The steps

**Step A. A new authoring surface over the present layout.** `piperine-abi` gains the new builder with
typed indices, the new traits and lenses, and `device_type::<D>`. They produce today's raw `Shape` and
`Module`. The present authoring surface stays. Only `piperine-abi` changes, so the tree is green.

The lowering that step A needs, item by item:

| New thing | Written to the present layout as |
| --- | --- |
| the three pin tables | one list `pins`: the parameters in the order of their names (rule P5 holds until step D), then the ports in the order of their positions (bind takes ports in pin order, `sv/core/bind.rs:623`–`:631`), then every other pin |
| `Linkage` | `role` and `global` of the pin; `OPTIONAL` as a pin flag |
| `STRUCTURAL`, `UNTRACKED`, `RELAXED_LOOP_ALLOWED` | the pin flags `STRUCTURAL`, `QUIET`, `RELAX`; `RECORD_STREAM` as `RECORD` |
| `WHERE_CONDITION` on pin `c` | `HAS_CHECK` on pin `c` with `check = c`; any pin may carry the flag (`abi/validate.rs:397`) and the engine reads only the check pin (`dg/engine.rs:559`–`:562`) |
| the bits of `present`; `SESSION_DEFAULT` | `HAS_INIT`, `count = 0`, the real `0.0`; the sentinel `DEFAULT` |
| a `minimum_delay` above zero | `LATER` with `min_after` |
| `limits`, `small_signal_sources` | one list `inputs`: the limits, then the sources, each with its kind |
| `crossings`, `operating_point_variables`, `traces` | one list `watches`, in that order, each with its kind |
| each write of `STEP_BOUND` | a `CAP` watch after those, named for the function and the write, and a write to it |
| `SMALL_SIGNAL_SOURCE_STRENGTH` of a stimulus | the two cells `RE` and `IM`; the generated entry splits the 16 bytes |
| `Read.element` | the parallel list `elements` |
| `EVENT_PIN_FIRED` | the part `CHANGED` of an event pin |
| a `ParameterValue` | a `Value` whose `pin` is the lowered index of the parameter |
| the merged `commit` | `commit` or `commit_element`, by whether the target is an array |

The table of functions is one list before and after, with the same function indices, and `evaluate` is
the present `eval`. Write indices are translated in a function that writes a stimulus strength, because
one write becomes two cells and every later write of that function moves by one; a `Partial.write`
that names a later write is translated with it, and the reading view of step C does the reverse. In
the tree such a function has no partial and no later write (`plan/plan.rs:808`).

**Step B. Producers move.** One commit per row; after each, both surfaces produce the same raw shape.

| Commit | Moves |
| --- | --- |
| B1 | `piperine-plan`, `piperine-transpile` (the printer; the golden files are regenerated here, once) and `piperine-interp`, together, because the loader compares the printed shape with the plan's (`tp/compiled.rs:69`) |
| B2 | the test devices of `piperine-abi`, except `tests/validate.rs` |
| B3 | the test devices of the analog solver |
| B4 | the test devices of the digital solver |
| B5 | the devices of `piperine-solver`, in its sources and in its tests |

Each hand-written device is rewritten once, in the commit that owns its file. 44 of those files also
write raw `Cell` literals in their function specifications; the same commit converts them to the
builder's typed reads and writes, so no file is visited twice.

The present authoring surface is **not** deleted here. `piperine-abi/tests/validate.rs` builds
malformed shapes with it to exercise the 23 present rules, and those rules keep running until step D.

**Step C. Consumers move to a reading view.** `piperine-abi` gains `ShapeView`, a read-only accessor in
the new vocabulary over the present raw shape. It covers everything a consumer names today:

- the eleven tables, as iterators of new-vocabulary entries, and the kind of each function (rule 4);
- the constants a consumer matches on: the aspects, including those of the call environment that
  replace `ambient::*` in `an/assembly.rs:681`–`:690` and `dg/run.rs:260`–`:265`, and the statuses;
- the call: `InstanceHandle::evaluate`, which is the present `Handle::eval` (used today at
  `an/assembly.rs:717`, `dg/run.rs:327` and `sv/analyses/sensitivity.rs:333`);
- both numberings of pins: the new index of a pin in its table, and the present pin index it lowers
  to, with a translation each way.

The last point is what makes the order of commits work. The owner builds one binding per *pin* and
hands the vector to each sub-solver (`sv/core/bind.rs:851`–`:882`; `an/plan.rs:36`–`:40`). While the
vectors are per pin, a sub-solver that has moved to the view looks a binding up by translating a
new index to the present one.

| Commit | Moves |
| --- | --- |
| C1 | the analog solver: its plan, its assembly and its accept step read the view; bindings stay per pin |
| C2 | the digital solver: its plan, its calls and the shadow read the view; bindings stay per pin |
| C3 | the owner: bind, the sensitivity analyses and the circuit read the view; it still builds per-pin vectors |
| C4 | **one commit across the three solver crates**: the binding vectors become three vectors, one per pin table, and the translation is no longer used |
| C5 | the transpiler's shape comparison and its loader |

C1, C2, C3 and C5 are one crate each. C4 is not, and the proposal does not claim it is: a type that
crosses three crates changes in one commit.

`piperine-api` and `piperine-parity` depend on `piperine-abi`. In their sources, `piperine-api` uses
only `raw::leaf` (`crates/piperine-api/src/table.rs:5`, `leaf_input.rs:6`), which does not change, and
neither crate implements a device. No change is expected in either; both are compiled and tested after
every commit of steps B to D. `piperine-cli` and `piperine-lang-server` do not depend on
`piperine-abi`.

**Step D. The raw layout flips.** In `piperine-abi`: `raw.rs` takes the structs of Part II; the builder
writes them directly and the view reads them directly; the lowering of step A and the translation of
step C are deleted; the present authoring surface is deleted; `validate` takes the 9 rules, and bind the check of one writer per strength;
`tests/validate.rs` is rewritten with raw literals, because the typed builder cannot write a malformed
shape, so tests inside `piperine-abi` do name raw structs; the layout test is re-pinned. No other crate
changes: producers name only the authoring surface and consumers only the view. The sources of
`piperine-abi` are embedded in the transpiler and hashed into the cache keys (`tp/compile.rs:43`,
`:78`–`:89`), so a library built before the flip has another key and is never found. Step D adds a test
of that.

**Step E. The seam, and the two word pairs.** Independent of A to D. The trait `DiscreteSide` becomes
`DigitalSide` and the adapter of that name becomes `DigitalSolverAdapter`; the bare `u32` of the seam
become `NetId`, `AnalogCopySlot`, `EventNetId` and `SampleIndex`; `Boundary` becomes `DeliveredChange`;
`Outcome`, `Advance`, `Attempt`, `Accepted` and `Shadow` gain the noun they are the result of; the two
structs `Ambient` become `SolveEnvironment` and `InstantEnvironment`; the owner's `Site` becomes
`BoundNet`; `piperine_solver::abi` becomes `piperine_solver::circuit`; `analyses/adapters.rs` moves to
`protocol/`. The solvers' `Binding` and `AnalogBinding` keep their names: they are what bind made of a
linkage.

### 7.3 Acceptance, and the alternative

No step changes what is computed, so **every numeric oracle passes with unchanged expected values
after every commit**. A difference is a defect of the migration. The six refusals of section 6 change
behaviour on purpose, in step D, and each gets a test.

A single cut-over was considered: one commit that changes `piperine-abi`, both generators, three solver
crates and 180 device implementations. It has no intermediate state that compiles, and a numeric
regression found afterwards cannot be bisected. It was rejected for that. The price of the staged order
is the temporary lowering and translation, written in steps A and C and deleted in D; the estimate is
some hundreds of lines and was not measured.

---

# Part II. Specification

This part is normative. The words MUST, MUST NOT, MAY and SHOULD are used as in RFC 2119. "The solver"
means the three solver crates together; "the producer" is whoever built a library or a shape; "the
caller" is the code that calls an entry point.

## 8. Conventions

**8.1 Layout.** Every struct of this part is `#[repr(C)]`, with its fields in the order listed. Sizes
and offsets are given for 64-bit targets and are pinned by a test of `piperine-abi`. A struct is shown
as its fields with the offset of each.

**8.2 Enumerations.** Every enumeration is a `u32` with named constants. The first constant of every
group is 1. Flags start at bit 0. A zero where an enumeration is expected is never valid.

**8.3 Indices.** An index is a `u32` position in a named table of the same shape, counted from 0.

**8.4 Absence.** There are two encodings, chosen by the type of the field and by nothing else.

- *A field that holds an index* is absent when it holds a named value that no index can equal: `GROUND`
  or `SESSION_DEFAULT` (section 13).
- *A field that holds a value* is absent when its bit of `present` is clear. Two structs have such
  fields, `ContinuousPin` and `DiscretePin`. Their optional fields are marked *optional 0*, *optional 1*
  and so on in the listing; bit *n* of `present: u32` set means that optional field *n* holds a value.
  When the bit is clear the field MUST be all zero (an empty `Bytes` for a byte field). A bit beyond the
  last optional field MUST be clear.

A descriptive field that says nothing is empty: an empty `unit` or `origin`, a null `DeviceType.data`.

**8.5 Names.** Every entry of a table begins with `name: Str` and `origin: Str`. `origin` is a source
position, `file:line:column`, and MAY be empty.

**8.6 Primitive views.** These are unchanged from the present contract (`abi/raw.rs:25`–`:82`).

| Struct | Fields | Size |
| --- | --- | --- |
| `Str` | `pointer: *const u8` @0, `length: usize` @8. UTF-8, no terminator; the pointer is never null. | 16 |
| `Bytes` | `pointer: *const u8` @0, `length: usize` @8. Immutable bytes. | 16 |
| `List<T>` | `pointer: *const T` @0, `length: usize` @8, `stride: usize` @16. Element *i* is at `pointer + i·stride`. | 24 |
| `Layout` | `size: u32` @0, `alignment: u32` @4, `fields: List<Field>` @8 | 32 |
| `Field` | `name: Str` @0, `leaf: u32` @16, `offset: u32` @20, `count: u32` @24 | 32 |

`leaf` is one of `BOOL` 1, `U8` 2, `U16` 3, `U32` 4, `U64` 5, `I8` 6, `I16` 7, `I32` 8, `I64` 9, `F32`
10, `F64` 11, all little-endian. A **real** is a field of leaf `F64`. A **boolean at byte 0** is a field
of leaf `BOOL` and offset 0. The crate ships the layouts `BIT`, `BYTE`, `F64` and `NONE`.

## 9. Library and device type

```rust
#[repr(C)] pub struct Library {
    pub size: u32,                          // @0
    pub minor: u32,                         // @4
    pub name: Str,                          // @8
    pub build: Str,                         // @24
    pub device_types: List<DeviceType>,     // @40      size 64
}

#[repr(C)] pub struct DeviceType {
    pub name: Str,                          // @0
    pub default_shape: *const Shape,        // @16
    pub data: *const c_void,                // @24
    pub create: CreateFunction,             // @32
    pub destroy: DestroyFunction,           // @40
    pub evaluate: EvaluateFunction,         // @48
    pub react: ReactFunction,               // @56      size 64
}

#[repr(C)] pub struct ParameterValue { pub discrete_pin: u32 /* @0 */, pub bytes: Bytes /* @8 */ }   // size 24
#[repr(C)] pub struct Created { pub status: u32 /* @0 */, pub instance: *mut Instance /* @8 */,
                                pub shape: *const Shape /* @16 */ }                                // size 24
#[repr(C)] pub struct Instance { _opaque: [u8; 0] }
#[repr(C)] pub struct FailureMessage { pub code: u32 /* @0 */, pub length: u32 /* @4 */,
                                       pub text: [u8; 248] /* @8 */ }                              // size 256
```

**9.1** A library MUST export the symbol `piperine_abi_1`, a function of no argument that returns a
pointer to a `Library`. The library, its device types and their default shapes MUST stay valid and
unwritten until the process ends. A library MAY export symbols of other contracts; the transpiler's
`piperine_derive_3` is one, and is not part of this specification.

**9.2** `default_shape` MUST be a valid shape (section 17). It is the shape of an instance created
with an empty list of values.

**9.3** The four entry fields MUST NOT be null. A device type that has no function, or no reaction,
points the entry at code that returns `FAULT`.

**9.4** `data` is the producer's own and is not interpreted by the solver.

**9.5** A `ParameterValue` gives initial bytes to one parameter. `discrete_pin` is the index of that
parameter in `discrete_pins` of the *default shape*. Its bytes MUST have the size of that pin's layout;
for an array parameter, the size of one element or of every element. A parameter's identity is its name
(10.3); `create` takes an index because both parties hold the default shape, where each name has one
index, and no string need be compared per call.

## 10. The shape

```rust
#[repr(C)] pub struct Shape {
    pub size: u32,                                                   // @0
    // pins
    pub continuous_pins: List<ContinuousPin>,                        // @8
    pub discrete_pins: List<DiscretePin>,                            // @32
    pub event_pins: List<EventPin>,                                  // @56
    // the analog tables
    pub terms: List<Term>,                                           // @80
    pub limits: List<Limit>,                                         // @104
    pub small_signal_sources: List<SmallSignalSource>,               // @128
    pub crossings: List<Crossing>,                                   // @152
    pub operating_point_variables: List<OperatingPointVariable>,     // @176
    pub traces: List<Trace>,                                         // @200
    // code
    pub functions: List<Function>,                                   // @224
    pub reactions: List<Reaction>,                                   // @248      size 272
}
```

**10.1** A shape MUST be complete: the byte length of every read and write, and the validity of every
index and partial, are decidable from the shape alone.

**10.2** A shape depends on structure only. A producer SHOULD return one shape to every instance of
equal structure. A shape other than a default shape MUST stay valid until the last instance created
with it is destroyed.

**10.3** A parameter is identified, within its device type, by its name. Every shape of a device type
MUST have, for each parameter of the default shape, exactly one parameter of equal name, equal layout
and equal element count, and no other parameter. The caller checks this when `create` returns (section
15.2). In the tree today a parameter's layout is a function of its type alone
(`plan/plan.rs:588`–`:596`) and each generic instantiation is its own device type, so no present
producer is constrained by this rule.

## 11. Pins

A **pin** is the instance's end of one net of the circuit. The three tables are the three sorts of net.

```rust
#[repr(C)] pub struct ContinuousPin {
    pub name: Str,                  // @0
    pub origin: Str,                // @16
    pub unit: Str,                  // @32
    pub linkage: Linkage,           // @48
    pub present: u32,               // @60
    pub hold_potential: f64,        // @64   optional 0
    pub potential_tolerance: f64,   // @72   optional 1
    pub flow_tolerance: f64,        // @80   optional 2      size 88
}

#[repr(C)] pub struct DiscretePin {
    pub name: Str,                  // @0
    pub origin: Str,                // @16
    pub unit: Str,                  // @32
    pub layout: Layout,             // @48
    pub linkage: Linkage,           // @80
    pub present: u32,               // @92
    pub flags: u32,                 // @96   discrete_pin_flag::*
    pub initial_value: Bytes,       // @104  optional 0
    pub element_count: u64,         // @120  optional 1      size 128
}

#[repr(C)] pub struct EventPin {
    pub name: Str,                  // @0
    pub origin: Str,                // @16
    pub linkage: Linkage,           // @32      size 48
}
```

**11.1 Continuous pin.** Its net is one unknown of the circuit: a node potential or a branch flow. It
has one equation, to which terms contribute (section 13.1).

- `hold_potential` is the value of the language's `= value` on a node: it is where Newton starts, and
  during the operating point of a transient the unknown is tied to that potential by a potential
  source to ground, the *hold*, which is removed as the first step begins
  (`spec/part_v_execution.md:426`–`:430`).
- `potential_tolerance` is the absolute tolerance that applies when the pin is on a node, and
  `flow_tolerance` the one that applies when it is on a branch unknown. Each tightens the session's
  value and never loosens it. On a node shared by several instances the smallest applies. A private
  pin, and a port that the netlist leaves unconnected, is on a branch unknown of its own
  (`sv/core/bind.rs:638`, `:643`).

**11.2 Discrete pin.** Its net holds bytes of `layout`, which change only at a microstep. With
`element_count` present the net is an array of that many values of the layout, of which the solver
stores only those written; `element_count` MUST then be at least 1.

`initial_value` is what the net holds when an analysis starts from initial values; the owner commits it
at instant (0, 0). For an array it is one element, which every element starts with, or every element.

| `discrete_pin_flag::` | Bit | Meaning |
| --- | --- | --- |
| `UNTRACKED` | 1 | The store keeps no previous value and no change for the net. No function reads it. A commit to it never prolongs an instant. |
| `RECORD_STREAM` | 2 | Every commit to the net is delivered to the host as a record of the stream `<instance path>.<name>`, and the store tracks nothing for it. The flag is a strict superset of `UNTRACKED`: it does all that `UNTRACKED` does, and delivers records. |
| `RELAXED_LOOP_ALLOWED` | 4 | A dependency loop among digital functions MAY pass through the net and is solved by relaxation. |
| `STRUCTURAL` | 8 | The net refuses a host write during a run. On a parameter it states that every change selects a shape, and the owner re-creates the instances on each accepted write (16.3). |
| `WHERE_CONDITION` | 16 | The pin is a **where condition**: it MUST have a boolean at byte 0, and it *holds* when that byte is nonzero. Section 16.4 says when where conditions are read. |

The first four flags and the initial value describe the *net*. When several pins are on one net, the
net has a flag when any of them sets it, and takes its initial value from the first pin that has one
and is not a parameter.

**11.3 Event pin.** Its net holds no bytes. It *fires* at a microstep: whenever a commit to it applies
(a reaction's, the host's, or the one the solver makes for an accepted crossing), and whenever a
digital function writes a nonzero byte to it. An event pin has no options: it is always tracked, and no
dependency loop passes through it.

## 12. Linkage

```rust
#[repr(C)] pub struct Linkage {
    pub linkage_kind: u32,      // @0   linkage_kind::*
    pub port_position: u32,     // @4   PORT only
    pub flags: u32,             // @8   linkage_flag::OPTIONAL 1      size 12
}
```

The linkage of a pin names the net it is on. Thirteen nets can be named: one that belongs to the
instance, one that the netlist chooses, one that the host addresses by name, and the ten that the
session provides for the whole design, which are the **session nets**.

| `linkage_kind::` | Value | The net | Allowed in |
| --- | --- | --- | --- |
| `PRIVATE` | 1 | a net of the instance's own, which no other instance and no host write reaches | the three pin tables |
| `PORT` | 2 | the net of the netlist's connection number `port_position`: a **port** | the three pin tables |
| `PARAMETER` | 3 | a net that the host writes under the pin's name, one per instance or one shared by several (a model card): a **parameter** | `discrete_pins` |
| `TEMPERATURE` | 4 | the run's temperature | `discrete_pins` |
| `TEMPERATURE_GIVEN` | 5 | whether the run set its temperature | `discrete_pins` |
| `RESOLUTION` | 6 | the time resolution, in seconds | `discrete_pins` |
| `ANALYSIS_DC` | 7 | the analysis in course is a DC analysis | `discrete_pins` |
| `ANALYSIS_TRANSIENT` | 8 | … a transient | `discrete_pins` |
| `ANALYSIS_AC` | 9 | … an AC analysis | `discrete_pins` |
| `ANALYSIS_NOISE` | 10 | … a noise analysis | `discrete_pins` |
| `START` | 11 | fires once when an analysis starts from initial values | `event_pins` |
| `SETTLE` | 12 | fires in the settle phase of a time point | `event_pins` |
| `SESSION_PARAMETER` | 13 | the session parameter whose name is the pin's name | `discrete_pins` |

`Linkage` is a tagged union and is specified as one: `port_position` belongs to the kind `PORT` and
MUST be 0 under every other kind. Two kinds use the pin's `name` as data: `PARAMETER` and
`SESSION_PARAMETER`. Under every other kind the name is descriptive.

**12.1 Ports.** The values of `port_position` in a shape, over the three tables together, MUST be the
integers from 0 to one less than the number of ports, each once. The netlist binds its *i*-th
connection to the port of position *i*. It MAY bind a discrete port to a whole net, or to a window of
consecutive elements of an array net: one element for a single-valued pin, `element_count` elements for
an array pin. Bind refuses a window of another length and an array net of another element count
(`sv/core/bind.rs:669`–`:701`). A port that the netlist leaves unconnected gets a net of its own.

**12.2 Parameters.** The parameter's name is the pin's name, its layout the pin's layout, and its
default the pin's initial value; a value the netlist or the host gives replaces the default. The host
writes a parameter by name; nothing else writes it. No two parameters of a shape have one name.

**12.3 Session nets.** Several pins, of one instance or of many, MAY be linked to one session net. The
layout of the pin MUST be one into which the owner can encode the net's value: a real for `TEMPERATURE`
and `RESOLUTION`; a boolean or a real for an `ANALYSIS_*` net, which the owner sets to true or to 1.0
(`sv/core/bind.rs:768`–`:772`). A `SESSION_PARAMETER` that the session does not provide is an error at
bind.

**12.4 `OPTIONAL`.** The flag says what happens when nothing provides the net: the pin is then
**absent**, and reads as zero bytes, in place of the outcome above (a net of its own for an unconnected
port; an error for a missing session parameter). Every other linkage always provides a net, and a
continuous pin is never absent, because an unconnected one is its own unknown; there the flag changes
nothing.

## 13. The analog tables

```rust
#[repr(C)] pub struct Term {
    pub name: Str,              // @0
    pub origin: Str,            // @16
    pub operator: u32,          // @32  term_operator::DIRECT 1 | TIME_DERIVATIVE 2 | DELAY 3
    pub positive: u32,          // @36  a continuous pin, or GROUND
    pub negative: u32,          // @40  a continuous pin, or GROUND
    pub state_tolerance: u32,   // @44  a discrete pin holding a real at byte 0, or SESSION_DEFAULT
    pub maximum_delay: u32,     // @48  a discrete pin holding a real at byte 0, or SESSION_DEFAULT      size 56
}

#[repr(C)] pub struct Limit { pub name: Str /* @0 */, pub origin: Str /* @16 */ }   // size 32

#[repr(C)] pub struct SmallSignalSource {
    pub name: Str,                      // @0
    pub origin: Str,                    // @16
    pub small_signal_source_kind: u32,  // @32  small_signal_source_kind::NOISE 1 | STIMULUS 2      size 40
}

#[repr(C)] pub struct Crossing {
    pub name: Str,              // @0
    pub origin: Str,            // @16
    pub event_pin: u32,         // @32  index into event_pins
    pub time_tolerance: u32,    // @36  a discrete pin holding a real at byte 0, or SESSION_DEFAULT
    pub value_tolerance: u32,   // @40  a discrete pin holding a real at byte 0, or SESSION_DEFAULT      size 48
}

#[repr(C)] pub struct OperatingPointVariable {
    pub name: Str /* @0 */, pub origin: Str /* @16 */, pub layout: Layout /* @32 */ }   // size 64

#[repr(C)] pub struct Trace {
    pub name: Str /* @0 */, pub origin: Str /* @16 */, pub layout: Layout /* @32 */ }   // size 64
```

Two named values stand where an index is expected and there is none. `GROUND` is `u32::MAX` and stands
for the reference node, which has no equation. `SESSION_DEFAULT` is `u32::MAX − 1` and stands for the
session's own tolerance or bound. Neither can be mistaken for an index.

**13.1 Terms.** The equation of an unknown *x* is

> Σ (direct terms) + d/dt Σ (time-derivative terms) + Σ (delay terms) = 0

over every term of every instance that names a pin on *x*, each counted with a plus sign where it names
the pin as `positive` and a minus sign where it names it as `negative`.

| `term_operator::` | The solver does | Reads |
| --- | --- | --- |
| `DIRECT` | adds the value | — |
| `TIME_DERIVATIVE` | differentiates the value in time, keeps its history, and tests truncation against `state_tolerance` | `state_tolerance` |
| `DELAY` | uses the value as it was `TERM_DELAY` seconds earlier, from a history it keeps up to `maximum_delay` | `maximum_delay` |

Both index fields are validated whatever the operator, and each is read only by the operator that the
table gives, as today. A delay greater than `maximum_delay` fails the solve.

**13.2 Limits.** A limit has four aspects (section 14): the value `x̂` held for the present iteration,
the value held before it, its age, and the proposed value `x`. A function that limits reads the
previous value and the age and writes the proposed value and the value. Every other function reads the
value, computes at `x̂`, and declares a partial *g* against it; the solver adds *g·(x − x̂)* to the term
and uses *g·∂x/∂unknowns* as the derivative. The solve has not converged while `|x̂ − x|` exceeds the
limiting tolerance. A limit that no function wrote in an iteration is answered with the held value for
all three. In a small-signal analysis `x̂ = x`.

An analog function flagged `OWNS_LIMIT_CORRECTION` writes a limit and the terms that depend on it in
one call, declares no partial against that limit, and reports values already linearized; the solver
then applies only the convergence test. On a function that writes no limit the flag says nothing.

**13.3 Small-signal sources.** The value of a small-signal source reads as 0 in every solve. A function
adds it where the source acts and declares a partial against it. At most one function of the shape
writes its strength; bind refuses a second writer, as it refuses a second writer of a term's value. The
analysis that the kind names injects the strength through the declared partials.

| `small_signal_source_kind::` | Value | The strength | Injected by |
| --- | --- | --- | --- |
| `NOISE` | 1 | one real: a spectral density. The source is a **noise source**. | a noise analysis, as one column per source; the report is keyed by `<instance path>.<name>` |
| `STIMULUS` | 2 | two reals: the real and the imaginary part of a phasor. The source is a **stimulus source**. | an AC analysis |

A strength that no function writes is 0.

**13.4 Crossings.** A crossing has a real value. When it goes from non-positive at one accepted
solution to positive at the next, the solver commits a firing of `event_pin` at microstep 0 of the
accepted tick, having located the crossing in time within `time_tolerance` and in value within
`value_tolerance`. A value that is not a number does not fire.

**13.5 Operating-point variables.** At each accepted solution the solver stores the value that the
variable's function wrote, and the host reads it by instance path and name.

**13.6 Traces.** A trace records evaluations, not solutions, and four rules follow from that.

1. Before every call of a function, the caller MUST zero the write buffer of each trace the function
   writes (`an/assembly.rs:712`–`:714`).
2. After the call, a buffer that is not all zero is a record. It is delivered to the host with the
   number of the Newton iteration. A producer SHOULD begin the layout with a boolean that it sets.
3. A trace record is not part of a checkpoint. A restore keeps it, and the host receives the records of
   rejected attempts too, once an acceptance reaches their time (`sv/protocol/sink.rs:1`–`:8`, `:22`).
4. A trace never causes a call. A function is called because a solve needs another of its writes; a
   function whose only writes are traces is never called (`an/assembly.rs:192`–`:193`).

## 14. Code, reads, writes and aspects

```rust
#[repr(C)] pub struct Read    { pub aspect: u32 /* @0 */, pub index: u32 /* @4 */, pub element: u64 /* @8 */ }  // size 16
#[repr(C)] pub struct Write   { pub aspect: u32 /* @0 */, pub index: u32 /* @4 */ }                              // size 8
#[repr(C)] pub struct Partial { pub write: u32, pub write_offset: u32, pub read: u32, pub read_offset: u32 }     // size 16

#[repr(C)] pub struct Function {
    pub name: Str,              // @0
    pub origin: Str,            // @16
    pub flags: u32,             // @32  function_flag::OWNS_LIMIT_CORRECTION 1
    pub reads: List<Read>,      // @40
    pub writes: List<Write>,    // @64
    pub partials: List<Partial>,// @88      size 112
}

#[repr(C)] pub struct Reaction {
    pub name: Str,                      // @0
    pub origin: Str,                    // @16
    pub flags: u32,                     // @32  reaction_flag::IRREVERSIBLE 1
    pub triggering_events: List<u32>,   // @40  indices into event_pins
    pub reads: List<Read>,              // @64
    pub commit_sites: List<CommitSite>, // @88      size 112
}

#[repr(C)] pub struct CommitSite {
    pub target: Write,          // @0   aspect DISCRETE_PIN_VALUE or EVENT_PIN_FIRED
    pub offset: u32,            // @8   the first byte commits through the site may write
    pub length: u32,            // @12  the number of such bytes; an event pin has none
    pub flags: u32,             // @16  commit_site_flag::REPLACES_PENDING 1
    pub minimum_delay: f64,     // @24  seconds      size 32
}
```

**14.1 Aspects.** An aspect is one facet of the entries of one table. *Reading* an aspect gives the
facet's present state, and *writing* it sets that state; an aspect never names two facets. Every aspect
names at most one table: eight aspects name none, the seven of the call environment and the step bound. `Read.index` and
`Write.index` index the aspect's table; for an aspect with no table the index MUST be 0.

`Read.element` chooses an element of an array and is taken only by the three aspects of a discrete pin;
it MUST be 0 elsewhere. It is 0 for a single value. For an array it is a constant element below the
element count, or `WHOLE_ARRAY` (`u64::MAX`), in which case the read holds an `ArrayView` (15.6).

| `aspect::` | Value | Table | Bytes | Read | Written |
| --- | --- | --- | --- | --- | --- |
| `CONTINUOUS_PIN_VALUE` | 1 | `continuous_pins` | 8 | the iterate; in a reaction, the accepted solution of its instant | — |
| `DISCRETE_PIN_VALUE` | 2 | `discrete_pins` | the layout | its bytes | its new bytes |
| `DISCRETE_PIN_PREVIOUS` | 3 | `discrete_pins` | the layout | its bytes before its latest change | — |
| `DISCRETE_PIN_CHANGED` | 4 | `discrete_pins` | 1 | 1 when it changed in this microstep | — |
| `EVENT_PIN_FIRED` | 5 | `event_pins` | 1 | 1 when it fired in this microstep | nonzero fires it |
| `TERM_VALUE` | 6 | `terms` | 8 | — | the term's value |
| `TERM_DELAY` | 7 | `terms` | 8 | — | the delay, in seconds, of a `DELAY` term |
| `LIMIT_VALUE` | 8 | `limits` | 8 | `x̂` | `x̂` |
| `LIMIT_PREVIOUS` | 9 | `limits` | 8 | the value held before this iteration | — |
| `LIMIT_AGE` | 10 | `limits` | 1 | 0 none held; 1 from the last accepted solution; 2 from the previous iteration | — |
| `LIMIT_PROPOSED` | 11 | `limits` | 8 | — | `x` |
| `SMALL_SIGNAL_SOURCE_VALUE` | 12 | `small_signal_sources` | 8 | 0 | — |
| `SMALL_SIGNAL_SOURCE_STRENGTH` | 13 | `small_signal_sources` | 8 for `NOISE`, 16 for `STIMULUS` | — | the strength (13.3) |
| `CROSSING_VALUE` | 14 | `crossings` | 8 | — | the value |
| `OPERATING_POINT_VARIABLE_VALUE` | 15 | `operating_point_variables` | the layout | — | the value |
| `TRACE_RECORD` | 16 | `traces` | the layout | — | the record |
| `TIME` | 17 | — | 8 | the time of the solve or instant, in seconds; 0 where time does not exist | — |
| `FREQUENCY` | 18 | — | 8 | the frequency of the sweep point; else 0 | — |
| `EQUILIBRIUM` | 19 | — | 1 | 1 in an equilibrium solve, or an instant of one | — |
| `MINIMUM_CONDUCTANCE` | 20 | — | 8 | the continuation conductance; at rest, the session's | — |
| `SOURCE_SCALE` | 21 | — | 8 | the continuation scale of the independent sources; at rest, 1 | — |
| `TICK` | 22 | — | 8 (`I64`) | the last tick not after the instant | — |
| `HORIZON` | 23 | — | 8 (`I64`) | the last tick through which nothing that triggers the reaction, or that it reads, can change | — |
| `STEP_BOUND` | 24 | — | 8 | — | a **step bound**: the next transient step is at most this many seconds |

Aspects 1 to 16 are named `<TABLE>_<FACET>`. Aspects 17 to 23 are the **call environment**: the values
that belong to one call and to no net. `TIME`, `FREQUENCY`, `MINIMUM_CONDUCTANCE`, `SOURCE_SCALE` and
`EQUILIBRIUM` are the language's `now()`, `freq()`, `gmin()`, `source_scale()` and `analysis(Static)`.
The environment of an instant is not that of a solve: in an instant the two continuation values are at
rest.

Aspect 24, `STEP_BOUND`, is not part of the environment. It is a result of an analog function, like a
term's value, that needs no table because the solver keeps nothing for it but the smallest: the
language's `bound_step(dt)`. A function MAY write it more than once, each write with its own buffer. At
an accepted solution the solver takes the smallest of every step bound that the kept results of the
functions hold. Positive infinity bounds nothing; any other value MUST be finite and positive, and the
fault names the function.

**14.2 The two kinds of function.** A function is **analog** when it writes an analog table or
`STEP_BOUND`, and **digital** when it writes a pin. A function MUST NOT do both. The kind is not
stored: it is this definition, and `piperine-abi` implements it once for every consumer.

Two sorts of function are never called. One that writes nothing is neither kind; its reads are checked
for known constants and indices in range (V2, V3) and for nothing else. This is looser than
today's validator, which applies the read rules of a function to it (`abi/validate.rs:524`–`:530`,
`:588`–`:594`), and it costs nothing, because neither solver calls a function that writes nothing.
One whose only writes are
traces is analog, and no solve needs it (13.6).

| | Analog function | Digital function |
| --- | --- | --- |
| Called by | the analog solver | the digital solver |
| Called when | a solve needs one of its writes other than a trace, and the bytes it reads differ from those of its last call | at a microstep, in dependency order, when something it reads changed; once at the first instant of an analysis from initial values; and on the shadow store |
| Partials wanted | those against continuous pins, limits and small-signal sources; every one in a sensitivity analysis | none; every one in a sensitivity analysis |
| Statuses | `OK`, `FAULT` | `OK`, `FAULT`, and `RESHAPE` under 16.3 |
| A fault is reported with | the instance path, the function's name and origin | the tick and the instance path |

**14.3 Who may use an aspect.** A read, a write or a commit target outside this table is refused (rule
V8).

| Aspect | Analog function | Digital function | Reaction |
| --- | --- | --- | --- |
| `CONTINUOUS_PIN_VALUE` | read | — | read |
| `DISCRETE_PIN_VALUE` | read | read, write | read; commit target |
| `DISCRETE_PIN_PREVIOUS` | read | read | read |
| `DISCRETE_PIN_CHANGED` | — | read, under condition 1 | read |
| `EVENT_PIN_FIRED` | — | read, under condition 1; write | read; commit target |
| `TERM_VALUE`, `LIMIT_PROPOSED`, `SMALL_SIGNAL_SOURCE_STRENGTH`, `CROSSING_VALUE`, `OPERATING_POINT_VARIABLE_VALUE`, `TRACE_RECORD`, `STEP_BOUND` | write | — | — |
| `TERM_DELAY` | write, on a term whose operator is `DELAY` | — | — |
| `LIMIT_VALUE` | read, write | — | — |
| `LIMIT_PREVIOUS`, `LIMIT_AGE`, `SMALL_SIGNAL_SOURCE_VALUE` | read | — | — |
| `TIME`, `EQUILIBRIUM` | read | — | read |
| `FREQUENCY` | read | — | — |
| `MINIMUM_CONDUCTANCE`, `SOURCE_SCALE` | read | read | read |
| `TICK`, `HORIZON` | — | — | read |

Three conditions belong to the same rule.

1. *A change produces only an occurrence.* A digital function that reads `DISCRETE_PIN_CHANGED` or
   `EVENT_PIN_FIRED` MUST write nothing but `EVENT_PIN_FIRED`. A change is visible for one microstep,
   so a value computed from it would outlive what it was computed from.
2. *Untracked nets.* No function reads any aspect of a discrete pin flagged `UNTRACKED` or
   `RECORD_STREAM`, and a reaction reads only its value.
3. *No arrays in analog code.* An analog function MUST NOT read a discrete pin that is an array.

Condition 3 costs no capability. The elaborator never emits such a read: for each array element that an
`analog` block reads it makes a single-valued mirror, defined as that element, and the analog code
reads the mirror (`crates/piperine-lang/src/elab/elements.rs:220`–`:282`). The plan gains an assertion
of the same rule.

**14.4 Partials.** A partial declares that the function computes ∂ `writes[write]` at byte
`write_offset` / ∂ `reads[read]` at byte `read_offset`. Both bytes MUST be the start of a real. The
partials of a function are exact on the declared pattern. The solver reads the pattern at bind and
never again.

**14.5 Commit sites.** A reaction makes commits only through its commit sites. A site names its target
as a `Write` does, and gives the byte range that commits through it MAY write; the range lies inside
the target's bytes, so it is empty for an event pin. `REPLACES_PENDING`: each commit through the site
removes every pending commit of the site. `minimum_delay` is a promise that every commit through the
site has a delay of at least that many seconds. A promise under one tick of the session, zero included,
promises nothing and is ignored at bind; a commit that breaks a kept promise is a fault.

## 15. Entry points

```rust
#[repr(C)] pub struct FunctionFrame {
    pub size: u32,                      // @0
    pub reads: *const *const u8,        // @8   one pointer per read
    pub writes: *const *mut u8,         // @16  one pointer per write
    pub wanted: *const u8,              // @24  one byte per partial: 1 when the caller wants it
    pub partials: *mut f64,             // @32  one real per partial, zeroed
    pub message: *mut FailureMessage,   // @40      size 48
}

#[repr(C)] pub struct ReactionFrame {
    pub size: u32,                          // @0
    pub reads: *const *const u8,            // @8
    pub commit_queue: *const CommitQueue,   // @16
    pub commit_queue_context: *mut c_void,  // @24
    pub message: *mut FailureMessage,       // @32      size 40
}

#[repr(C)] pub struct CommitQueue {
    pub size: u32,                      // @0
    pub commit: CommitFunction,         // @8       size 16
}

#[repr(C)] pub struct ArrayView {
    pub context: *const c_void,         // @0
    pub element_count: u64,             // @8
    pub element_size: u32,              // @16
    pub read: ArrayReadFunction,        // @24      size 32
}
```

| Type | Signature |
| --- | --- |
| `CreateFunction` | `unsafe extern "C" fn(*const DeviceType, List<ParameterValue>, *mut FailureMessage) -> Created` |
| `DestroyFunction` | `unsafe extern "C" fn(*mut Instance)` |
| `EvaluateFunction` | `unsafe extern "C" fn(*mut Instance, index: u32, *const FunctionFrame) -> u32` |
| `ReactFunction` | `unsafe extern "C" fn(*mut Instance, index: u32, *const ReactionFrame) -> u32` |
| `CommitFunction` | `unsafe extern "C" fn(context: *mut c_void, commit_site: u32, element: u64, offset: u32, length: u32, bytes: *const u8, after: f64) -> u32` |
| `ArrayReadFunction` | `unsafe extern "C" fn(context: *const c_void, element: u64, aspect: u32) -> *const u8` |

The statuses are `OK` 1, `FAULT` 2, `RESHAPE` 3 and `FINISH` 4.

The device has five entries (`piperine_abi_1`, `create`, `destroy`, `evaluate`, `react`) and the solver
two (`commit`, `ArrayView.read`). The struct `CommitQueue` is the reaction's handle on the run-time
area of the same name (section 4); the two share the name on purpose.

### 15.1 Rules common to every entry

**15.1.1** No entry may let a panic or an exception cross the boundary, in either direction.

**15.1.2** Every pointer of a frame is valid for the duration of the call only. The device MUST NOT
keep one.

**15.1.3** `reads[i]` and `writes[j]` carry **no alignment guarantee**. The device MUST read and write
through them by copy. `wanted` and `partials` are aligned arrays of `u8` and `f64`. Each read and write
buffer has exactly the byte length that the shape gives its aspect.

**15.1.4** The caller zeroes `message` before the call. The device writes it only when it returns
`FAULT` or `RESHAPE`: a code of its own choosing, and up to 248 bytes of UTF-8 text.

**15.1.5** A reader of a frame or of a `CommitQueue` MUST NOT read a field at or beyond its `size`
(section 18).

### 15.2 `create`

*Preconditions.* `device_type` is a live device type. Every value names a parameter of the default
shape and has the byte length of 9.5.

*Postconditions.* On `OK`, `instance` is non-null and `shape` is a valid shape that obeys 10.3. The
device has kept no pointer into the values.

*Statuses.* `OK`, `FAULT`. A device type that cannot build an instance for these values returns
`FAULT`. `create` never returns `RESHAPE`: it returns the shape that fits the values.

*The caller's checks.* The caller (`InstanceHandle::create`) MUST validate the returned shape (section
17) and MUST check it against the default shape as 10.3 requires, before any other call on the
instance. These checks are the caller's, so that they hold for a library written in any language; the
Rust thunk on the device's side MAY repeat them to fail earlier.

### 15.3 `destroy`

*Precondition.* No call on the instance is in progress, and none follows. *Postcondition.* The
instance is freed and its reference to its shape released.

### 15.4 `evaluate`

`evaluate(instance, index, frame)` runs `functions[index]`.

*Preconditions.* `index` is in range. The frame holds one read pointer per entry of `reads`, filled
with the bytes of that aspect; one write pointer per entry of `writes`; and `wanted` and `partials`
with one entry per entry of the function's `partials`. A write buffer holds the bytes that the last
call left in it, or the present state of the facet for a pin. There is one exception: the buffer of a
`TRACE_RECORD` is all zero at every call (13.6).

*Postconditions.* On `OK`, every write buffer holds the function's output, and `partials[p]` holds the
exact derivative for every `p` with `wanted[p] = 1`; an unwanted entry MAY be left zero.

*Purity.* The writes and the partials MUST depend only on the bytes read. An instance MAY keep scratch
and caches that nothing can observe. The solver relies on this to skip a call whose read bytes did not
change, to call twice, to call on a copy of its storage, and to call in any order.

*Statuses.* `OK` and `FAULT`; and `RESHAPE` from a digital function, under the two conditions of 16.3.

*Callers.* The analog solver calls the analog functions, inside solves and at accepted solutions. The
digital solver calls the digital functions, at instants and on the shadow store. The sensitivity
analysis calls both, with every partial wanted (`an/assembly.rs:587`; `sv/analyses/sensitivity.rs:333`).
Section 14.2 has the schedule of each.

### 15.5 `react`

`react(instance, index, frame)` runs `reactions[index]`.

*Preconditions.* An event of `triggering_events` fired in the present microstep. The reads are those
of a snapshot taken before any reaction of the microstep ran. `commit_queue` and its context are valid
for this call only.

*Postconditions.* The reaction's only effect is the commits it made. It MUST be a function of its
reads: it carries nothing from one call to the next, except through discrete pins it reads and
commits to.

*Repetition.* The solver MAY call a reaction more than once in one microstep, with a smaller `HORIZON`
each time. Only the commits of the last call stand.

*Statuses.* `OK`; `FAULT`; `FINISH`, which ends the analysis after the present instant.

*`IRREVERSIBLE`.* A reaction whose effect the solver cannot undo, because it touches state outside the
run-time data areas, MUST carry this flag. The solver then never runs it ahead of the accepted analog
time, gives it an exact horizon, and calls it once.

### 15.6 `CommitQueue.commit` and `ArrayView.read`

`commit(context, commit_site, element, offset, length, bytes, after)` asks for a commit through
`commit_sites[commit_site]` of the calling reaction.

*Preconditions.* It is called inside the `react` call that received this queue. `offset` and `length`
lie inside the site's declared range. `element` is 0 for a single value and for an event pin, and below the
element count for an array. `after` is not NaN, and is at least the site's kept `minimum_delay`.

*Effect.* With `after ≤ 0`, or a delay that rounds to no tick, the commit is due at the next microstep.
Otherwise it is due at microstep 0 of the tick nearest to the present time plus `after`. Positive
infinity is never due.

*Statuses.* `OK`, `FAULT`. A refused commit fails the reaction even when the device ignores the status.

`read(context, element, aspect)` returns the bytes of one element of an array, as the call's snapshot
has it, for `DISCRETE_PIN_VALUE`, `DISCRETE_PIN_PREVIOUS` or `DISCRETE_PIN_CHANGED`
(one byte). The pointer is valid until the call that holds the view returns.

## 16. Execution rules

**16.1 Order.** Digital functions run in dependency order, when something they read changed, and once
at the first instant of an analysis from initial values. A dependency loop is legal only through
nets flagged `RELAXED_LOOP_ALLOWED`. The order of the reactions of one microstep MUST NOT
be observable.

**16.2 Commits.** Commits to the same bytes that apply in one microstep are ordered by (the instant
each was made, the index of its commit site, the order of the calls); the last is the value. A commit
removes the pending commits of its own site, to overlapping bytes of the same element, that are due at
or after its own due instant. One writer is allowed per net, and per element of an array net: the
digital function that writes it, or the instance whose commit sites name it, or else the host.

**16.3 `RESHAPE` and `STRUCTURAL`.** There are two ways for a parameter to require another shape. A
parameter is `STRUCTURAL` when every change of it selects a shape; the producer knows this statically,
and the owner re-creates the instances on each accepted write without asking the device. A digital
function answers `RESHAPE` when one particular value of an ordinary parameter, or of a session net,
does not fit the present shape; only the device can tell.

`RESHAPE` is legal when two conditions hold together, as today (`dg/run.rs:328`):

1. the function lies inside the parameter cone;
2. the call is made on the shadow store, to try a host write.

In every other call, and from any other entry, `RESHAPE` is a fault that names the function. Because
every host write to a parameter net or a session net is tried on the shadow first, a write that would
need another shape is found there. The owner refuses such a write inside a transient; between analyses
it destroys, re-creates and re-binds the instances.

**16.4 Host writes and where conditions.** Before the solver accepts a host write to a parameter net or
a session net, it MUST run the parameter cone downstream of that net on a copy of the discrete store
and then read **every where condition of every instance** of the design. If any does not hold, the
write is refused, nothing changes, and the refusal names the instance and the condition's pin.

**16.5 Finite values.** A value that is not finite, in anything the present solve reads (a term, a
delay, a limit, a wanted partial), fails the solve with the instance, the entry and its origin. A
crossing value that is not a number does not fire; an infinite one fails the solve.

## 17. Validation

A shape MUST pass every rule below before the solver interprets it. The rules are checked in this
order, so that no list is walked before it is known to be readable and no constant is interpreted
before it is known. Each rule is one sort of defect, and the right column lists every case of it.

| # | Rule | A shape is refused when |
| --- | --- | --- |
| V1 | `UnreadableList` | a list's stride is under the element's size, breaks its alignment, or its pointer is misaligned |
| V2 | `UnrecognizedConstant` | a constant is outside the group its field allows, or a flag or presence bit is not defined |
| V3 | `IndexOutOfRange` | an index is past the end of what it indexes: (a) a table; (b) the elements of an array, which includes `WHOLE_ARRAY` on a single value; (c) the bytes of a layout, for a field or for the byte range of a commit site |
| V4 | `NotApplicable` | something is set where the listing says it MUST be zero: (a) an optional field whose presence bit is clear; (b) `Read.element` on an aspect that is not of a discrete pin; (c) an index on an aspect that names no table; (d) `port_position` on a linkage that is not `PORT` |
| V5 | `LinkageNotAllowed` | a linkage kind is used in a table that the table of section 12 does not give it |
| V6 | `LinkageRepeated` | (a) the port positions are not the integers from 0, each once; (b) two parameters have one name |
| V7 | `ValueNotAsRequired` | bytes do not have the form their use requires: (a) a partial's write or read byte is not the start of a real; (b) a tolerance or bound of a term or a crossing names a discrete pin without a real at byte 0; (c) a `WHERE_CONDITION` pin has no boolean at byte 0; (d) an array's initial value is neither one element nor every element long; (e) an element count is 0 |
| V8 | `AccessNotAllowed` | a read, a write or a commit target is outside the table and the three conditions of 14.3; or a function writes both a pin and an analog table (14.2) |
| V9 | `LimitCorrection` | a function writes a limit and a term (a) without `OWNS_LIMIT_CORRECTION`, or (b) with it and a partial against that limit |

That is nine rules. Eight of them have twenty-two cases between them, counting the three of V1 and the
three of V2; the ninth, V8, is the table of 14.3, of thirteen rows and three conditions, and one
sentence of 14.2.

**How the count compares.** The present validator has 23 rules (`abi/validate.rs:18`–`:98`).

| Fate | Present rules | Count |
| --- | --- | --- |
| Unwritable in the new tables, deleted | `TermTargetNotContinuous`, `ReactionOnNonEvent`, `ParamAfterNonParam`, `CrossEventNotEvent`, `EventNotEmpty`, `ContinuousNotF64`, `ArrayNotDiscrete` | 7 |
| Merged into V8 | `MixedSorts`, `DiscreteReadsAnalog`, `ChangedReadByWriter`, `QuietRead`, `QuietChangeRead`, `StatementOnContinuous` | 6 into 1 |
| Merged into V7 | `PartialNotF64`, `TermBoundNotDiscreteF64`, `CheckNotBool`, `ArrayInitLength` | 4 into 1 |
| Merged into V3 | `IndexOutOfRange`, `ElementOutOfRange` | 2 into 1 |
| Kept one to one: V1, V2, V5, V9 | `UnreadableList`, `UnknownConstant`, `KindRole`, `LimitCorrection` | 4 |
| New: V4, V6 | — | 2 |

A merged rule is not fewer checks: it is one name for one sort of defect, as the present validator
already uses `IndexOutOfRange` for a table, a layout field and a byte range (`abi/validate.rs:418`,
`:642`). The honest measure of what there is to learn is the number of cases, and it is given above.

**Checks that need more than one shape** are made elsewhere, as their counterparts are today:

- by the caller when `create` returns: the parameters of the created shape against those of the default
  shape (10.3, 15.2);
- at bind: one writer per net; one writer per term value and per strength of a small-signal source
  (`an/plan.rs:460`–`:468` for terms; new for strengths, refusal 3); equal layouts on a net; an array
  pin on an array net of the same element count, and on a window of its own length (12.1); loops only
  through nets flagged `RELAXED_LOOP_ALLOWED`; the encodability of session nets (12.3).

## 18. Versioning

**18.1** The major version is the suffix of the symbol `piperine_abi_1`. The minor version is
`Library.minor`. Until the first release the contract changes in place at major 1 and minor 0, and this
proposal is such a change. A compiled library can never be loaded against another contract than the
one it was built with: the sources of `piperine-abi` are hashed into the keys of the transpiler's cache
(`tp/compile.rs:43`, `:78`–`:89`), and a library is opened only if its bytes hash to what the cache
recorded (`tp/load.rs:57`–`:62`).

**18.2** After the first release, a minor version MAY append a field to a struct of the first two
classes below, add a constant to a group, and add a table at the end of `Shape`. It MUST NOT change
anything else.

| Class | Members | Rule |
| --- | --- | --- |
| Passed by pointer, with `size` | `Library`, `Shape`, `FunctionFrame`, `ReactionFrame`, `CommitQueue` | A reader MUST NOT read a field at or beyond `size`. A field not reached is absent, and the version that appends a field MUST state what its absence means. |
| Element of a `List` | every table entry, `DeviceType`, `Read`, `Partial`, `CommitSite`, `ParameterValue` | The producer's `stride` governs. A reader skips what lies beyond the element it knows, and MUST refuse a list whose stride is under that element (V1). |
| Embedded by value, without `size` | `Str`, `Bytes`, `List`, `Layout`, `Linkage`, `Write`, `Created`, `ArrayView` | Frozen. A change is a major version. |

**18.3** A loader MUST refuse a library whose `size` is under the `Library` it reads, or whose `minor`
is newer than its own. An unrecognized constant from a newer producer is refused by V2, naming it. An
older device never receives a constant it does not know, because everything it is given was declared by
its own shape.

**18.4 Two worked cases**, for a field appended to `CommitQueue` after a release.

- *Older solver, newer device.* The solver passes a `CommitQueue` whose `size` stops before the new
  field. The device sees that `size` does not reach it and treats it as absent; a device that cannot
  work without it returns `FAULT` with a message.
- *Newer solver, older device.* The device was compiled with the shorter struct and never reads the
  new field. The solver MUST NOT depend on the device using it.

For a field appended to a table entry, an older solver reads a newer library by skipping the tail of
each element; a newer solver refuses an older library's list. This asymmetry is the present rule and
is kept.

## 19. Threading

**19.1** No two calls on one instance overlap. The caller serializes them.

**19.2** Calls on different instances MAY run on different threads at the same time. A library MUST
NOT have mutable global state.

**19.3** `commit` and `ArrayView.read` are called only inside the call that received them, on the
thread of that call.

**19.4** A shape, a library and a device type are immutable and MAY be read from any thread.

## 20. Error reporting

| Where | What is reported |
| --- | --- |
| A shape refused by validation | the rule, the entry (table and name), its origin, and a detail |
| A shape refused against the default shape | the instance path, the parameter number and what differs |
| `FAULT` from `create` | the build of the circuit fails with the device type, the code and the text of the message |
| `FAULT` from `evaluate`, on an analog function | the analysis fails with the instance path, the function's name and origin, the code and the text |
| `FAULT` from `evaluate` on a digital function, or from `react` | the analysis fails with the tick, the instance path, the code and the text |
| A status that the entry or the context does not allow | a fault that names the instance, the entry and the status |
| A refused commit | the reaction fails with the instance path, the commit site and the reason |
| A non-finite value | error `E0507`, with the instance path, the entry and its origin |
| A refused host write | the instance path and the name of the where condition that does not hold; or the instance that would need another shape |

A device written in Rust with the crate's helpers meets 15.1.1 without effort: the generated entry
catches a panic and returns `FAULT` with code 0 and the panic's text.

### 20.1 The Rust helpers (informative)

The crate keeps its present helpers under the new names. They are not part of the binary contract.

| Present | Proposed | Note |
| --- | --- | --- |
| trait `Device` | trait `DeviceImplementation` | a Rust type that implements one device type; its values are the instances |
| trait `Factory` | trait `DeviceFactory` | a Rust value that implements one device type from data |
| lenses `Eval`, `React` | `FunctionFrameLens`, `ReactionFrameLens` | a safe view of the frame of the same name |
| `Flow::{Continue, Finish}` | `ReactionOutcome::{ContinueAnalysis, FinishAnalysis}` | |
| `Fault::reshape` | `Fault::needs_another_shape` | every method returns `Result<_, Fault>`; the generated entry sends `RESHAPE` only from `evaluate` of a digital function and turns it into `FAULT` everywhere else |
| `get::<T>`, `out::<T>` (typed borrows) | removed | with no alignment guarantee a borrow would work or fail by accident of the caller's packing; the copying accessors `read::<T>(i) -> T` and `write(j, value)` remain |
| `Values`, `Pod`, `ShapeBuf`, `ShapeRef`, `Handle` | `ParameterValues`, `PlainData`, `ShapeBuilder`, `SharedShape`, `InstanceHandle` | |
| `module::<D>`, `module_with`, `library` | `device_type::<D>`, `device_type_with`, `library` | no macro |
| `Commit` (caller side) | `RequestedCommit` | it is pending only once the queue holds it |

The builder returns typed indices (`ContinuousPinIndex`, `DiscretePinIndex`, `EventPinIndex`,
`TermIndex`, and so on), each a struct of one field, so that passing a discrete pin where a term
wants a continuous one does not compile.

---

# Part III. Tables

## 21. Mapping from the present contract

### 21.1 Structs and fields

| Present | Proposed |
| --- | --- |
| `Str`, `Bytes`, `BytesMut`, `List<T>`, `Layout`, `Field`, `Created`, `Instance` | unchanged |
| `Library.modules` | `Library.device_types` |
| `Module { name, prototype, data, create, destroy, eval, react }` | `DeviceType { name, default_shape, data, create, destroy, evaluate, react }` |
| `CreateFn`, `DestroyFn`, `EvalFn`, `ReactFn` | `CreateFunction`, `DestroyFunction`, `EvaluateFunction`, `ReactFunction` |
| `CommitFn`, `CommitElementFn`, `ArrayReadFn` | `CommitFunction`, `ArrayReadFunction` |
| `Value { pin, bytes }` | `ParameterValue { discrete_pin, bytes }`; the index is into the default shape |
| `Note { code, len, text }`; `NOTE_TEXT` | `FailureMessage { code, length, text }`; the capacity stays 248 |
| `Shape`, six lists | `Shape`, eleven lists |
| `Pin` of kind `CONTINUOUS` | `ContinuousPin` |
| `Pin` of kind `DISCRETE` | `DiscretePin` |
| `Pin` of kind `EVENT` | `EventPin` |
| `Pin.role`, and `Pin.global` for the role `GLOBAL` | `Linkage.linkage_kind` |
| the position of a `PORT` pin among the `PORT` pins | `Linkage.port_position` |
| the position of a `PARAM` pin (rule P5) | nothing: a parameter is identified by its name (10.3) |
| `Pin.init` with `HAS_INIT`, continuous | `ContinuousPin.hold_potential` |
| `Pin.init` with `HAS_INIT`, discrete, any role | `DiscretePin.initial_value` |
| `Pin.check` with `HAS_CHECK` | the flag `WHERE_CONDITION` on the discrete pin that `check` named; which pin carried `HAS_CHECK` is not kept |
| `Pin.abstol[0]`, `Pin.abstol[1]` | `potential_tolerance`, `flow_tolerance` |
| `Pin.count` | `DiscretePin.element_count` |
| `Pin.layout`, `Pin.unit` | on `DiscretePin`; `unit` also on `ContinuousPin`; the unit of an event pin is dropped |
| `Term { op, plus, minus, abstol, max }` | `Term { operator, positive, negative, state_tolerance, maximum_delay }` |
| `Input` of kind `LIMIT` | `Limit` |
| `Input` of kind `NOISE`, `STIM` | `SmallSignalSource` of kind `NOISE`, `STIMULUS` |
| `Watch` of kind `CROSS`, with `event`, `ttol`, `etol` | `Crossing { event_pin, time_tolerance, value_tolerance }` |
| `Watch` of kind `VAR` | `OperatingPointVariable` |
| `Watch` of kind `TRACE` | `Trace` |
| `Watch` of kind `CAP` | no entry: a write of the aspect `STEP_BOUND` |
| `Cell { kind, index, part }` | `Read { aspect, index, element }`, `Write { aspect, index }` |
| `Function.elements`, `Reaction.elements` | `Read.element` |
| `Partial { write, to, read, at }` | `Partial { write, write_offset, read, read_offset }` |
| `Function { flags, reads, writes, partials, elements }` | `Function { flags, reads, writes, partials }` |
| `Reaction { flags, on, reads, writes, elements }` | `Reaction { flags, triggering_events, reads, commit_sites }` |
| `Statement { pin, offset, len, flags, min_after }` | `CommitSite { target, offset, length, flags, minimum_delay }` |
| `Eval { size, input, output, seed, jac, note }` | `FunctionFrame { size, reads, writes, wanted, partials, message }` |
| `React { size, input, host, host_data, note }` | `ReactionFrame { size, reads, commit_queue, commit_queue_context, message }` |
| `Host { size, commit, commit_element }` | `CommitQueue { size, commit }` |
| `ArrayView { context, count, size, read }` | `ArrayView { context, element_count, element_size, read }` |
| `Pod`, `pod_ref`, `pod_mut`, `pod_bytes` | `PlainData`, `plain_data_reference`, `plain_data_mutable`, `plain_data_bytes` |

### 21.2 Constants

| Present | Proposed |
| --- | --- |
| `kind::CONTINUOUS`, `DISCRETE`, `EVENT` | the three pin tables |
| `role::PRIVATE`, `PORT`, `PARAM` | `linkage_kind::PRIVATE`, `PORT`, `PARAMETER` |
| `role::GLOBAL` with `global::TEMPERATURE`, `RESOLUTION`, `START`, `SETTLE`, `TEMPERATURE_GIVEN` | the same names in `linkage_kind` |
| … with `global::DC`, `TRAN`, `AC`, `NOISE` | `linkage_kind::ANALYSIS_DC`, `ANALYSIS_TRANSIENT`, `ANALYSIS_AC`, `ANALYSIS_NOISE` |
| … with `global::NAMED` | `linkage_kind::SESSION_PARAMETER` |
| `pin_flag::OPTIONAL` | `linkage_flag::OPTIONAL` |
| `pin_flag::HAS_INIT` | bit 0 of `present` |
| `pin_flag::HAS_CHECK` | `discrete_pin_flag::WHERE_CONDITION` |
| `pin_flag::QUIET`, `RECORD`, `RELAX`, `STRUCTURAL` | `discrete_pin_flag::UNTRACKED`, `RECORD_STREAM`, `RELAXED_LOOP_ALLOWED`, `STRUCTURAL` |
| `leaf::*`, `status::*` | unchanged |
| `op::FLOW`, `DDT`, `DELAY` | `term_operator::DIRECT`, `TIME_DERIVATIVE`, `DELAY` |
| `input::LIMIT` | the table `limits` |
| `input::NOISE`, `STIM` | `small_signal_source_kind::NOISE`, `STIMULUS` |
| `watch::CROSS` | the table `crossings` |
| `watch::VAR`, `TRACE` | the tables `operating_point_variables`, `traces` |
| `watch::CAP` | `aspect::STEP_BOUND` |
| `cell::PIN` + `part::VALUE` | `aspect::CONTINUOUS_PIN_VALUE`, `DISCRETE_PIN_VALUE` |
| `cell::PIN` + `part::PREVIOUS` | `aspect::DISCRETE_PIN_PREVIOUS` |
| `cell::PIN` + `part::CHANGED`, on a discrete pin | `aspect::DISCRETE_PIN_CHANGED` |
| `cell::PIN` + `part::CHANGED`, on an event pin, read or written | `aspect::EVENT_PIN_FIRED` |
| `cell::TERM` + `part::VALUE`, `part::TIME` | `aspect::TERM_VALUE`, `TERM_DELAY` |
| `cell::INPUT` + `part::VALUE` | `aspect::LIMIT_VALUE`, `SMALL_SIGNAL_SOURCE_VALUE` |
| `cell::INPUT` + `part::LAST`, `AGE`, `PROPOSED`, `LIMITED` | `aspect::LIMIT_PREVIOUS`, `LIMIT_AGE`, `LIMIT_PROPOSED`, `LIMIT_VALUE` |
| `cell::INPUT` + `part::PSD`; `part::RE` and `IM` together | `aspect::SMALL_SIGNAL_SOURCE_STRENGTH`, of 8 bytes; of 16 bytes |
| `cell::WATCH` + `part::VALUE` | `aspect::CROSSING_VALUE`, `OPERATING_POINT_VARIABLE_VALUE`, `TRACE_RECORD`, `STEP_BOUND` |
| `cell::AMBIENT` + `ambient::TIME`, `FREQ`, `STATIC`, `GMIN`, `SOURCE_SCALE`, `TICK`, `HORIZON` | `aspect::TIME`, `FREQUENCY`, `EQUILIBRIUM`, `MINIMUM_CONDUCTANCE`, `SOURCE_SCALE`, `TICK`, `HORIZON` |
| `fn_flag::OWNS_CORRECTION` | `function_flag::OWNS_LIMIT_CORRECTION` |
| `reaction_flag::IRREVERSIBLE` | unchanged |
| `statement_flag::LATER` with `min_after` | a `minimum_delay` above zero |
| `statement_flag::REPLACE` | `commit_site_flag::REPLACES_PENDING` |
| `GROUND` | unchanged |
| `DEFAULT` | `SESSION_DEFAULT` |
| `WHOLE` | `WHOLE_ARRAY` |
| `MAJOR`, `MINOR`, `SYMBOL` | unchanged |

## 22. What exists today, and how the proposal carries it

The first five tables are the inventory: every capability of the present ABI and protocol that the
code proves, with the line that proves it. Section 22.7 then gives, for every number, the mechanism of
the proposal. The inventory uses the present names, because it describes the present code.

### 22.1 Analog

| # | Capability | Proof |
| --- | --- | --- |
| C1 | A term added to one row and subtracted from another, in Norton form (`FLOW`). | `abi/raw.rs:812`; `an/flow.rs:105` |
| C2 | A term the solver differentiates in time (`DDT`): history, companion current, a per-term tolerance for the truncation test. | `an/ddt.rs:59`, `:89`, `:109`; `abi/raw.rs:540` |
| C3 | A term delayed by a delay the device writes (`DELAY`), bounded by a per-term maximum; history kept by the solver. | `an/delay.rs:20`, `:29`, `:68` |
| C4 | Either side of a term may be the reference node. | `abi/raw.rs:711` |
| C5 | Declared partials; the sparsity pattern is built from declarations at bind (direct partials, and partials through a limit's `PROPOSED`). | `abi/raw.rs:596`; `an/plan.rs:210`, `:474`–`:493` |
| C6 | The solver asks for a subset of the partials per call. Newton asks for those against continuous pins and held inputs. | `an/plan.rs:499`–`:508` |
| C7 | Memoized evaluation by comparison of the packed read bytes. | `an/assembly.rs:698`–`:724` |
| C8 | A function is called only by the solve that needs one of its outputs (rows, limits, stimulus, noise, accepted-solution watches). A trace is recorded when its function is called and never makes a solve need it. *Corrected during the review.* | `an/plan.rs:509`–`:530`; `an/assembly.rs:192`–`:193`, `:654`–`:659` |
| C9 | Junction limiting with a solver-held iterate (`VALUE`, `LAST`, `AGE`, `PROPOSED`, `LIMITED`), the correction, the convergence veto. | `an/assembly.rs:43`–`:54`; `an/limit.rs:21`, `:40`, `:64`, `:80`, `:94` |
| C10 | A limit no function wrote in an iteration is answered from the held value. | `an/limit.rs:47` |
| C11 | A foreign model that limits inside its own call (`OWNS_CORRECTION`). | `abi/raw.rs:869`; `abi/validate.rs:35`–`:37` |
| C12 | Noise sources: zero in the solution, a density written by a function, injected through declared partials. | `an/small_signal.rs:72`, `:152` |
| C13 | AC stimulus: zero in the solution, a phasor written by a function, injected through declared partials. | `an/small_signal.rs:65`, `:89`, `:165`, `:194` |
| C14 | Operating-point variables with a layout, stored at accepted solutions, read by path and name. | `an/watch.rs:77`; `an/assembly.rs:288`, `:296` |
| C15 | Crossing detectors: `≤ 0` to `> 0` fires an event; located in the step by secant with the Illinois rule; per-crossing time and value tolerances. | `an/watch.rs:53`, `:77`; `an/assembly.rs:263`; `sv/analyses/transient.rs:1082`, `:1714` |
| C16 | Step caps. | `an/watch.rs:77`; `sv/protocol/sides.rs:300` |
| C17 | Traces per evaluation, stamped with the Newton iteration; the buffer is zeroed before each call, and the records survive a rejected step. *Completed during the review.* | `an/watch.rs:136`; `an/assembly.rs:313`, `:712`–`:714`; `sv/protocol/sides.rs:219`; `sv/protocol/sink.rs:22` |
| C18 | The environment of a solve: time, frequency, equilibrium flag, `gmin`, source scale. | `an/assembly.rs:28`–`:39`, `:681`–`:690` |
| C19 | The environment of an instant: tick, time, equilibrium flag, horizon, and `gmin` and source scale as the owner set them. | `dg/run.rs:25`–`:37`; `dg/engine.rs:209` |
| C20 | A non-finite value in a cell the solve reads fails it (`E0507`), naming instance, cell and origin. | `an/assembly.rs:728`–`:760` |
| C21 | Per-pin tolerances from the nature: the potential tolerance tightens the node the pin is bound to, the flow tolerance tightens the branch unknown it is bound to; folded by minimum over the pins. An initial value held at the operating point. | `sv/core/bind.rs:645`–`:662`, `:359`–`:374`; `plan/plan.rs:622` |
| C22 | An analog function reads discrete values and their previous values from the analog side's own copy. | `an/plan.rs:44`–`:60`, `:271`; `an/assembly.rs:168` |
| C23 | Private unknowns (branch flows of potential contributions, hoisted operators). | `sv/core/bind.rs:643`; `plan/plan.rs:637`–`:638` |
| C24 | Small-signal operator responses. | `an/small_signal.rs:24` |
| C25 | The host reads a term's value by path and name. | `an/assembly.rs:305` |
| C26 | Partials against parameters, and partials of discrete functions, are seeded by the sensitivity analysis; the fallback is a finite difference. | `an/assembly.rs:535`–`:586`; `sv/analyses/sensitivity.rs:331`–`:333`, `:59` |

### 22.2 Discrete

| # | Capability | Proof |
| --- | --- | --- |
| C27 | Three net kinds: continuous, discrete, event. | `abi/raw.rs:754`; `abi/validate.rs:367`–`:371` |
| C28 | Four ways to bind a pin: by position, by name, by constant, allocated. An unconnected port is allocated, or absent when `OPTIONAL`. | `sv/core/bind.rs:623`–`:643`, `:702`–`:722` |
| C29 | Ten design-wide nets, including a named session parameter. | `abi/raw.rs:769`–`:783` |
| C30 | Pin options `STRUCTURAL`, `OPTIONAL`, `HAS_INIT`, `HAS_CHECK`, `QUIET`, `RECORD`, `RELAX`. | `abi/raw.rs:786`–`:794` |
| C31 | Layouts over eleven leaves; layout equality; leaf sizes. | `abi/raw.rs:519`, `:528`, `:797`; `abi/layout.rs:27`, `:39` |
| C32 | Array nets: sparse storage, a scalar pin on one element, an array pin on a window, whole-array views, an element chosen at run time on commit. | `dg/array.rs:54`; `dg/plan.rs:20`–`:32`; `sv/core/bind.rs:42`, `:683`–`:701`; `abi/raw.rs:695`; `dg/run.rs:303`–`:313` |
| C33 | Discrete functions run in dependency order when a read changed. | `dg/run.rs:365`, `:397` |
| C34 | Loops through `RELAX` nets are relaxed, bounded, `E0511`. | `dg/run.rs:437`, `:495` |
| C35 | Value, previous value and changed flag of a net; an event fired by a function. | `dg/store.rs:226`, `:270`, `:276`, `:285`; `dg/run.rs:146` |
| C36 | One writer per net, per element of an array net, derived at bind. | `dg/plan.rs:597`–`:638` |
| C37 | Reactions on events, on one snapshot. | `dg/run.rs:533`, `:598`, `:765` |
| C38 | Declared commit ranges; a commit outside its declaration is refused. | `abi/raw.rs:639`; `dg/run.rs:644`–`:660` |
| C39 | `LATER` with `min_after`: kept at bind only when at least one tick; a broken promise is a fault. | `dg/plan.rs:291`; `dg/run.rs:662`–`:668` |
| C40 | Ordering, transport removal and `REPLACE`. | `dg/queue.rs:38`–`:56` |
| C41 | Delay rules: NaN refused (`E0435`), `+∞` never, non-positive or zero ticks is the next microstep. | `dg/tick.rs:47`–`:66` |
| C42 | The microstep loop, bounded at 1000, `E0501`. | `dg/instant.rs:115`, `:221`; `dg/limits.rs:16` |
| C43 | Quiet nets; record nets deliver each commit as a record. | `dg/run.rs:114`–`:116`, `:708`–`:709` |
| C44 | A reaction reads the accepted continuous solution. | `dg/plan.rs:28`–`:29`; `sv/protocol/sides.rs:163` |
| C45 | An absent optional pin reads as zero. | `dg/plan.rs:30`–`:31`; `an/plan.rs:269` |
| C46 | Events are nets: a statement on an event has no bytes and two such commits overlap; a located crossing is scheduled on its event. | `dg/queue.rs:50`–`:55`; `sv/protocol/sides.rs:234` |
| C47 | The settle phase and settle commits held for the next time point. | `dg/instant.rs:251`; `dg/settle.rs:23`, `:42` |

### 22.3 Protocol

| # | Capability | Proof |
| --- | --- | --- |
| C48 | Irreversible reactions run last and once; when one fires with others, every horizon is the instant. | `dg/run.rs:777`–`:795`; `dg/landing.rs:88` |
| C49 | Horizon, cut down and the reaction called again until a fixed point. | `dg/run.rs:797`–`:823`; `sv/protocol/sides.rs:71`; `sv/protocol/transient.rs:443` |
| C50 | Look-ahead that stops before a landing, with five causes. | `dg/advance.rs:28`; `sv/protocol/sides.rs:24`–`:35`, `:232` |
| C51 | Marks, restore and release on both sides and on the observation buffer. | `sv/protocol/sides.rs:262`–`:266`, `:332`–`:333`; `dg/store.rs:168`, `:174`; `sv/protocol/sink.rs:33`, `:38` |
| C52 | A step that must land earlier (`LandedEarlier`). | `sv/protocol/sides.rs:133`; `sv/analyses/transient.rs:1714` |
| C53 | Delivery of changed values with a restart mode; after a host parameter write the next step is a tenth of the last. | `sv/protocol/sides.rs:149`, `:323`; `sv/analyses/transient.rs:1924` |
| C54 | The zero-width solve over values passed through within an instant. | `dg/passing.rs:15`; `sv/protocol/retake.rs:132`; `sv/analyses/transient.rs:1849`, `:1878` |
| C55 | The retake of a step that read a jump. | `sv/protocol/retake.rs:27`; `sv/protocol/sides.rs:344` |
| C56 | Homotopy seen by devices only through `gmin` and source scale. | `sv/analyses/dc.rs:30`–`:34`, `:123` |
| C57 | The mixed operating point in at most 20 rounds, `E0506`. | `sv/protocol/operating_point.rs:24`, `:55`; `sv/protocol/transient.rs:56` |
| C58 | A reaction ends the analysis (`FINISH`). | `abi/raw.rs:885`; `sv/protocol/sides.rs:47` |
| C59 | Clock-origin rebasing. | `sv/protocol/transient.rs:229`; `sv/protocol/sides.rs:176`, `:357` |
| C60 | Idle instants are skipped. | `sv/protocol/sides.rs:244`, `:248` |
| C61 | A dynamic time-point test, and a static reference mode. | `dg/landing.rs:37`–`:40` |
| C62 | A run that starts at a later tick. | `sv/protocol/sides.rs:228`; `dg/engine.rs:229` |
| C63 | The protocol's statistics: what ended each step, zero-width causes. | `sv/protocol/statistics.rs:1`–`:25` |

### 22.4 Host

| # | Capability | Proof |
| --- | --- | --- |
| C64 | A host write to a parameter or global net runs the parameter cone on a copy of the store, and is refused when **any** check pin of the design reads false afterwards. | `dg/engine.rs:536`–`:567` |
| C65 | A `RESHAPE` from the cone is refused inside a transient and reported outside one; between analyses the owner re-creates and re-binds every instance. | `sv/protocol/host.rs:56`–`:74`; `sv/core/circuit.rs:254`–`:257`, `:328` |
| C66 | A `STRUCTURAL` parameter re-creates on every accepted `set` with no `RESHAPE` returned; a write to a structural net during a run is refused. | `sv/core/circuit.rs:230`, `:254`; `dg/engine.rs:376`–`:378` |
| C67 | A host write at a byte offset inside a net, checked on the shadow with the same offset. | `sv/protocol/host.rs:47`, `:89`; `dg/engine.rs:536` |
| C68 | A dry run of a `set` (`check_set`). | `sv/core/circuit.rs:266`–`:281` |
| C69 | A host write to a discrete net between analyses, kept as an initial write of every later analysis. | `sv/core/circuit.rs:297`–`:323`, `:249` |
| C70 | A host drive of a port, immediate or scheduled. | `piperine-api/src/host.rs:63`; `dg/engine.rs:366` |
| C71 | Host waiters on events. | `sv/protocol/host.rs:114`; `sv/protocol/sides.rs:276`; `dg/engine.rs:401` |
| C72 | Observation: changes, commits with reaction and statement, crossings, records (with or without a site), traces; a step observer. | `sv/protocol/sides.rs:206`–`:219`; `sv/protocol/sink.rs:13`; `sv/protocol/step.rs:13` |
| C73 | Parameter discovery from the prototype; values encoded against its layouts; a shared parameter net (model card). | `abi/thunk.rs:188`–`:213`; `sv/core/bind.rs:70`, `:709`–`:722` |
| C74 | Net properties folded over every pin on the net: `QUIET`, `RECORD`, `RELAX`, `STRUCTURAL` by OR; the initial value from the first pin that has one and is not a parameter. | `sv/core/bind.rs:495`–`:505` |
| C75 | Sensitivity refuses a structural parameter by its flag. | `sv/analyses/sens.rs:98`; `sv/analyses/sensitivity.rs:128` |
| C76 | The run's temperature, set by the host. | `sv/core/circuit.rs:204` |

### 22.5 ABI mechanics and the compiled backend

| # | Capability | Proof |
| --- | --- | --- |
| C77 | Versioning by symbol, `Library.size` and `minor` (checked by the loader), and `List.stride`. | `tp/load.rs:53`–`:73`; `abi/raw.rs:82`, `:282`, `:715`–`:719` |
| C78 | `Host.size` tells whether `commit_element` exists. | `abi/thunk.rs:359` |
| C79 | A failure note with a device-chosen code. | `abi/raw.rs:463`–`:470` |
| C80 | Statuses; a reshape fault is told from a plain fault. | `abi/raw.rs:885`; `abi/thunk.rs:95`–`:103` |
| C81 | No two calls on one instance overlap. | `abi/caller.rs:107`–`:127` |
| C82 | No unwinding across the boundary, either way. | `abi/thunk.rs:95`–`:100`; `abi/caller.rs:198`, `:401` |
| C83 | The Rust traits `Device` and `Factory`, lenses, `Pod`. | `abi/device.rs:16`, `:31`, `:133`, `:200`; `abi/raw.rs:903` |
| C84 | A shape builder that owns its memory; shapes shared by reference count. | `abi/shape.rs:17`, `:131`, `:338` |
| C85 | The caller's handle: create once, share, a packed evaluation that allocates nothing, destroy on last drop. | `abi/caller.rs:230`, `:259`, `:302`–`:326`, `:424` |
| C86 | Generated C entries with no macro; values checked against the prototype and the created shape validated in the thunk. | `abi/thunk.rs:26`, `:42`, `:56`, `:188`–`:216` |
| C87 | Shape-local validation, 23 rules. | `abi/validate.rs:18`–`:98`, `:272` |
| C88 | Finite-difference verification of partials. | `abi/verify.rs:42` |
| C89 | Per-module opaque data. | `abi/raw.rs:360`–`:372`, `:409` |
| C90 | **The load-time shape check.** The prototype of every compiled module is compared with the plan's shape entry by entry, before any instance is created. | `tp/compiled.rs:66`–`:71`; `tp/shape_check.rs:1`–`:27` |
| C91 | **A second exported symbol, `piperine_derive_3`**, that recomputes parameters derived from others; a library without it is refused when the design derives. | `tp/print/derive.rs:22`; `tp/load.rs:117`–`:130`; `tp/compiled.rs:74`–`:80` |
| C92 | The sources of `piperine-abi` are embedded in the transpiler, and generated code is built against them. | `tp/abi_source.rs:1`–`:20` |
| C93 | The C layout is pinned by a test. | `abi/raw.rs:971` |
| C94 | The analyses built on the linearized operating point, or on repeated transients, consume the same facets of a shape: transfer function, pole-zero, S-parameters, adjoint sensitivity, periodic steady state. | `sv/analyses/tf.rs:1`, `pz.rs:1`, `sp.rs:1`, `adjoint.rs:1`, `pss.rs:1` |
| C95 | The host reads which limits are still active. | `an/assembly.rs:212` |
| C96 | The noise report has one series per source name, so the name of a noise source is visible to the host. | `crates/piperine-solver/tests/noise_report.rs:1`–`:2` |
| C97 | An instant that ran to its end is reopened at its next or its last microstep. | `sv/protocol/sides.rs:239` |
| C98 | The net of an analysis kind may be declared with a boolean or with a real layout: bind encodes true, and falls back to 1.0. | `sv/core/bind.rs:768`–`:772` |

### 22.6 Specified and not implemented

| # | Capability | Status |
| --- | --- | --- |
| S1 | The owner's `force` and `release` overlay on a net (abi.md P8; terms of `spec/part_viii_host.md`). | There is no `force` in `piperine-solver`, `piperine-digital-solver` or `piperine-api`; the only `release` is the checkpoint's (`sv/protocol/sides.rs:266`, `dg/store.rs:196`). By its specification no device takes part, so the ABI is not involved. Under the project's rule that code follows the specification this is a defect, and it is decision 7 of Part IV. |

### 22.7 Coverage

"Unchanged" means that the behaviour lives in the solver and reads a renamed declaration. Every number
from C1 to C98, and S1, has a row.

| # | Mechanism in the proposal |
| --- | --- |
| C1–C4 | `Term` with `term_operator`; `TERM_VALUE`, `TERM_DELAY`; `state_tolerance`, `maximum_delay`, or `SESSION_DEFAULT`; `GROUND` (13.1) |
| C5, C6 | `Partial` (14.4); `FunctionFrame.wanted` (15.4); which partials each caller wants (14.2) |
| C7 | unchanged in the analog solver: the same comparison of the same packed read bytes; no read changes length |
| C8 | unchanged: an analog function is classified at bind by the aspects it writes: terms and limits; a strength, of a stimulus or of a noise source; and `CROSSING_VALUE`, `OPERATING_POINT_VARIABLE_VALUE` and `STEP_BOUND`, which are taken at accepted solutions (`an/plan.rs:509`–`:530`). `TRACE_RECORD` is in no class: a trace never causes a call (13.6, rule 4) |
| C9, C10, C11 | `Limit` and its four aspects; `OWNS_LIMIT_CORRECTION` (13.2) |
| C12 | `SmallSignalSource` of kind `NOISE`; `SMALL_SIGNAL_SOURCE_VALUE`; `SMALL_SIGNAL_SOURCE_STRENGTH` of one real (13.3) |
| C13 | `SmallSignalSource` of kind `STIMULUS`; `SMALL_SIGNAL_SOURCE_VALUE`; `SMALL_SIGNAL_SOURCE_STRENGTH` of two reals, written by one function as the generator does today (`plan/plan.rs:808`) |
| C14 | `OperatingPointVariable`, with its layout (13.5) |
| C15 | `Crossing` with its event and two tolerances (13.4) |
| C16 | the written aspect `STEP_BOUND` (14.1) |
| C17 | `Trace`, with the four rules of 13.6: zeroed before each call, stamped with the iteration, kept through a restore, never the cause of a call |
| C18, C19 | the seven aspects of the call environment (14.1) |
| C20 | unchanged (16.5) |
| C21 | `hold_potential`, `potential_tolerance`, `flow_tolerance`; folding by minimum unchanged (11.1) |
| C22 | `DISCRETE_PIN_VALUE` and `DISCRETE_PIN_PREVIOUS` read by an analog function (14.3) |
| C23 | a continuous pin whose linkage is `PRIVATE` |
| C24, C25 | unchanged |
| C26 | `Partial` on functions of both kinds; the one `evaluate` entry with every partial wanted, as the analysis calls the one `eval` today |
| C27 | the three pin tables (section 11) |
| C28 | `linkage_kind`: `PORT` by position, `PARAMETER` by name, a session net by constant, `PRIVATE` allocated; an unconnected port gets a net of its own, or is absent when `OPTIONAL` (section 12) |
| C29 | the ten session nets of `linkage_kind` (section 12) |
| C30 | `OPTIONAL` as the linkage flag; `HAS_INIT` as a presence bit; `HAS_CHECK` as `WHERE_CONDITION`; `STRUCTURAL`, `UNTRACKED`, `RECORD_STREAM`, `RELAXED_LOOP_ALLOWED` as flags of a discrete pin. Narrowed, and listed as refusal 2 of section 6: the flags and an initial value on an event pin |
| C31 | unchanged (8.6) |
| C32 | `element_count`; `Read.element`; `WHOLE_ARRAY` and `ArrayView`; the `element` of `commit`; a port bound to one element or to a window of an array net (12.1) |
| C33 | digital functions (14.2, 16.1) |
| C34 | `RELAXED_LOOP_ALLOWED` on discrete pins (16.1). Narrowed, and listed as refusal 2 of section 6: a loop through an event net |
| C35 | `DISCRETE_PIN_VALUE`, `_PREVIOUS`, `_CHANGED`; `EVENT_PIN_FIRED`, read and written |
| C36 | unchanged, derived from the writes of digital functions and from commit sites (16.2) |
| C37 | `Reaction.triggering_events` (15.5) |
| C38, C39, C40, C41 | `CommitSite`; `minimum_delay`; `REPLACES_PENDING`; the `after` argument (14.5, 15.6, 16.2) |
| C42 | unchanged |
| C43 | `UNTRACKED`, `RECORD_STREAM` |
| C44 | `CONTINUOUS_PIN_VALUE` read by a reaction |
| C45 | `OPTIONAL` (12.4) |
| C46 | `EventPin`; a commit site whose target is `EVENT_PIN_FIRED`, with an empty byte range; `Crossing.event_pin` |
| C47 | unchanged; `linkage_kind::SETTLE` |
| C48, C49 | `IRREVERSIBLE`; `HORIZON`; repetition of a reaction (15.5) |
| C50–C57 | unchanged: the protocol reads no name of the ABI that changes meaning |
| C58 | `FINISH` from `react` only (15.5) |
| C59–C63 | unchanged |
| C64 | the flag `WHERE_CONDITION`; every flagged pin of every instance is read after the cone (16.4) |
| C65 | the two conditions of 16.3, which are today's; the owner's handling unchanged |
| C66 | `discrete_pin_flag::STRUCTURAL` (11.2, 16.3) |
| C67, C68, C69, C70, C71 | unchanged |
| C72 | unchanged; a commit's observation names the reaction and the commit site |
| C73 | `DeviceType.default_shape`; `ParameterValue`; a shared parameter net is the owner's binding of a `PARAMETER` linkage (9.5, 12.2) |
| C74 | unchanged, and stated (11.2) |
| C75 | reads `STRUCTURAL` from the discrete pin |
| C76 | `linkage_kind::TEMPERATURE` |
| C77 | section 18 |
| C78 | `CommitQueue.size`; its one present use disappears with the merge of the two commit entries, and the mechanism stays |
| C79, C80 | `FailureMessage`; the statuses per entry (section 15) and, for `RESHAPE`, by the two conditions of 16.3 |
| C81 | section 19 |
| C82 | 15.1.1 |
| C83, C84, C85, C86 | the Rust helpers (20.1); the caller's checks move to `InstanceHandle::create` (15.2); the packed call is `evaluate` |
| C87 | section 17 |
| C88 | unchanged, over `evaluate` |
| C89 | `DeviceType.data` |
| C90 | `DeviceType.default_shape`, compared with the plan at load as today |
| C91 | unchanged; outside this contract (9.1) |
| C92 | unchanged; it is what makes step D safe (18.1) |
| C93 | re-pinned in step D |
| C94 | unchanged: these analyses call the analog functions through the same assembly |
| C95 | unchanged, keyed by the names of `limits` |
| C96 | `SmallSignalSource.name` of a noise source (13.3) |
| C97 | unchanged |
| C98 | kept: 12.3 allows a boolean or a real, and rule V5 fixes only the table |
| S1 | not implemented today; the ABI is not involved |

Two rows are narrower than today, C30 and C34, both by refusal 2 of section 6 (flags on an event pin),
and no producer in the tree uses what is refused. Inventory row C8 is corrected in this version: a trace
does not make a solve need a function.

Performance properties, one by one. The memo is the same byte comparison over the same packed buffer;
no read gains or loses a byte against today, and no padding is added. Sparsity is fixed at bind from
the same declarations: the partials of analog functions, directly and through `LIMIT_PROPOSED`. A
function is still called only by the solve that needs it. The instance has no memory, so restore,
retake, look-ahead and parallel evaluation need no entry. Shapes are shared by structure. The packed
call allocates nothing. `UNTRACKED` nets avoid change tracking and extra microsteps. A kept
`minimum_delay` removes landings. Arrays stay sparse. `Read` grows from 12 to 16 bytes; reads are
resolved once, at bind (`an/plan.rs:496`–`:498`), so no iteration or microstep reads one. The kind of a
function and the three tags are read at bind and copied into the solvers' plans, as `Input.kind` and
`Watch.kind` are today. One write changes length: a stimulus strength is one buffer of 16 bytes where
it was two of 8; write buffers are not part of the memo's comparison.

## 23. Divergences between documentation and code

The manual lists thirteen (its §8.3). The review found one more.

| # | Divergence | Fate |
| --- | --- | --- |
| 1 | abi.md R5 lets a horizon reach the end of the tentative step; the code stops at the instant while there are unknowns. | Out of scope: behaviour of the protocol. |
| 2 | abi.md §12 promises aligned buffers; the analog solver passes packed bytes. | **Closed** on the side of the code (15.1.3). Decision 6 of Part IV. |
| 3 | abi.md P6 re-creates "that instance"; the code re-creates all. | Out of scope. |
| 4 | abi.md counts six entry points; the code has eight. | **Closed**: section 15 counts five and two. |
| 5 | Only two of the five `size` fields are read. | **Closed**: 15.1.5 and 18.2. |
| 6 | Tools promised by abi.md do not exist. | Out of scope. |
| 7 | abi.md R1 says a reaction runs once per microstep; it may be called again. | **Closed**: 15.5. |
| 8 | Comments say a step of 1e-3 after a host write; the code uses a tenth. | Out of scope: a stale comment. |
| 9 | A whole-array read is accepted in any function. | **Closed**: 14.3 and rule V8. |
| 10 | abi.md says nothing was compiled. | Closed when abi.md is rewritten. |
| 11 | A comment describes a removed file. | Out of scope. |
| 12 | `SPEC_DEVIATION` comments left as accepted state. | Out of scope, except `analyses/adapters.rs`, moved in step E. |
| 13 | The record of MD-84 still reads "pending review". | Housekeeping note in Part IV. |
| 14 | `abi/raw.rs:509` documents the tolerance pair as "unknown, row"; the consumer uses it as potential and flow (D12). | **Closed**: 11.1. |

---

# Part IV. Decisions for the user

Eight decisions. Each changes something the user has said, or something the user will have to live
with. For each: what is proposed, what it costs, and what happens on the other side. Two of them (6 and
7) ask for an exception to the project's rule that code follows the specification, and say so.

**Decision 1. One rule for *continuous/discrete* and *analog/digital*.**
*Recommended:* the language's own rule (section 2.1). *Continuous* and *discrete* describe what is
held; *analog* and *digital* describe code and the solver that runs it. The crates keep their names,
and MD-60 stands as written.
*Cost:* the seam trait `DiscreteSide` becomes `DigitalSide`, and the adapter that has that name today
becomes `DigitalSolverAdapter` (step E). `AnalogVariable`, with 113 uses, keeps its name as the analog
solver's own type.
*The other side:* one pair everywhere. With *continuous/discrete* everywhere, two crates, their
directories, `AnalogVariable` and `Digital` are renamed, MD-60 is amended, and the language's `analog`
and `digital` blocks are then run by solvers of another name. With *analog/digital* everywhere, a pin
that holds a real parameter is called "digital", against the language's "discrete carrier".

**Decision 2. The names.**
The user's second complaint was that names did not match what things do, so the words are the user's
call. Four are proposed; the rest are words of the language, used with the language's meaning (section
3.2).

| Word | For | Why this word | The other side |
| --- | --- | --- | --- |
| **pin** (kept) | the instance's end of a net; three tables | It is the present word with the present meaning, in about 100 source files. The defect of the present `Pin` was a struct that answered two questions with twelve fields, and that is what changes. **Its cost:** in electronics a pin is an external terminal, and here an internal node, a parameter, a where condition and the tolerance of a `ddt` are pins too. They are pins only in the ABI's sense, "the instance's end of a net". The present ABI has the same usage, so nothing gets worse, and nothing gets better either. | *variable*: it is the language's word for a `let`, and an operating-point variable would then not be a "variable". *Unknown* for the continuous one alone: accurate, but leaves no common word for the three. |
| **linkage** | the field that names a pin's net | Free in the language and in the tree; the solvers keep *binding* for what bind made of it. | *role* (today): says nothing of nets. |
| **small-signal source** | the table of noise and stimulus sources | The field's own term; the tree has `SmallSignal`. | two tables under the language's two words; one table and one aspect more, one tag fewer. |
| **where condition** (for today's `check`) | a flag of a discrete pin | The language's word. A *check* in the language is a record on the `checks` stream. | keep "check" in messages (`sv/core/circuit.rs:237`) and accept one word for two things across two layers. |

*Cost:* user-visible error text changes for the last row.

**Decision 3. Where to stop merging: eleven tables.**
*Recommended:* the tables of section 0.3. Each row below is one choice and can be taken or refused on
its own.

| Choice | If refused |
| --- | --- |
| the linkage as a field of the pin, with the ten session nets as values of its tag | three interface tables come back, with a rule that a pin is declared at most once |
| noise and stimulus sources in one table | decision 2, third row |
| a where condition as a flag | a table of entries that are one index each |
| a step bound as a written aspect with no entry | a twelfth table of entries that hold a name; error `E0509` then names the site and not the function |
| operating-point variables and traces as two tables | one table with a tag that decides when the value is taken, whether the buffer is zeroed, whether a rejection keeps it and whether it can cause a call |
| a parameter identified by its name | a parameter number in the linkage |

*Cost:* eleven tables where the present ABI has six.
*The other side, towards fewer:* three further merges were examined by the proposer and by the critic,
and both judged them unsound: crossings into another table, events into discrete pins, and one table of
pins. Each brings back a tag that decides which fields an entry has.
*The other side, towards more:* one table per final user of a value, eighteen in all, which is the
version the user rejected.

**Decision 4. The kind of a function: derived or declared.**
The complaint against the present ABI was that a function's kind is "told apart by inspection".
*Recommended:* derived. One table of functions and one `evaluate` entry; a function is digital when it
writes pins and analog when it writes analog tables, by one definition implemented once (14.2). Nothing
can contradict the writes, and there is one tag fewer.
*Cost:* the inspection remains. A reader of a shape looks at a function's writes to learn which solver
calls it, what it may read and which statuses it may return; adding one write can change what the
function may read.
*The other side:* a declared kind, one `u32` in `Function`. A reader then sees the kind at a glance and
the validator checks the writes against it; the cost is a fourth tag and a new way for a shape to be
inconsistent. Going further, two tables and two entry points make the statuses of the analog entry
exactly `OK` and `FAULT`, at the price of a table and an entry.

**Decision 5. Six refusals.**
*Recommended:* accept all six. The table of section 6 is complete and gives, for each, what happens
today and the evidence that nothing in the tree depends on it. In short: an array read by analog code;
flags or an initial value on an event pin; a strength written by two functions; a session net
linked from the wrong table; two parameters of one name; an element on a read that takes none. The
first is a missing guard, the second and third remove something that works today and that no producer
uses, and the last three never worked.
*Cost:* a producer outside the tree that relies on the second or the third is refused.
*The other side:* for the second, give `EventPin` the four flags and an initial value, which adds a
field and a group of flags to a struct that otherwise has none. For the third, keep two aspects for the
two parts of a phasor and let the last writer win, as today.

**Decision 6. The alignment promise is withdrawn: an exception to "code follows the specification".**
*Recommended:* read and write pointers carry no alignment, and a device reads by copy (15.1.3). The
typed borrows of the Rust lens are removed. This changes the specification to say what the code has
always done, which the project's rule forbids as a default; the recommendation asks for the exception
because the other side costs time inside the Newton loop.
*Cost:* an author in C must copy.
*The other side:* aligned buffers. The analog solver's packed byte buffer and its write buffers become
an aligned type, with padding written on every fill and compared in every memo test.

**Decision 7. `force` and `release`: implement, or strike. Also an exception.**
The language specification and abi.md P8 define an owner's overlay on a net. No code implements it.
*Recommended:* strike it from abi.md now and track it as an open item of the host surface. The language
specification would then keep a term that the implementation lacks, which under the project's rule is a
defect and not an accepted state; the recommendation asks the user to accept it knowingly until the
feature is scheduled. The device ABI is not involved either way.
*The other side:* implement it as part of this work, which adds a feature of the owner and of the host
API to a change that is otherwise a restructuring.

**Decision 8. A staged migration, and the rename done once, now.**
*Recommended:* the five steps of section 7, with the renames of Part III in the same steps, while the
ABI is unreleased and changes in place. The tree is green after every commit, each device is rewritten
once, and one commit (C4) crosses three crates. abi.md is rewritten as the specification of record from
Part II, and the manual is regenerated from it.
*Cost:* temporary code written in steps A and C and deleted in D; every document that names the ABI is
rewritten.
*The other side:* a single cut-over, in one commit that cannot be bisected; or the new structure under
the present names, which leaves `Cell`, `Input` and `Watch` naming things that no longer exist.

### Notes that are not decisions

- **Settled early in the review and carried.** `commit` stays, as the language's word. There is no separate table of
  parameters: the default shape describes them, and the loader's check against the plan (C90) uses it.
- **Not options, because the other side loses a capability.** An analysis net may be a boolean or a
  real, as bind accepts today (C98). A digital function that reads no change may write values and
  events together, as it may today (`abi/validate.rs:547`–`:571`).
- **`piperine_derive_3` stays outside the device ABI**, as a contract between the transpiler and its
  own output (9.1).
- **Analog code never reads an array.** The elaborator guarantees it; the proposal adds the guard that
  the validator and the plan lack (14.3).
- **Housekeeping.** The record of decision MD-84 (`.specs/STATE.md:1943`) still reads "pending user
  review", while the zero-width solve it describes is in `origin/main` and its amendment of the
  language specification is marked approved. The marker is stale; it is unrelated to this proposal.

---

# Appendix. The review

The proposal was written by a proposer and attacked by a critic, both citing one snapshot of the code.
A first review of two rounds produced the third version, which the user rejected for its quantity. A
second review of three rounds, scored on three axes, produced this one. The findings of every round and
the answer to each are in `proposal-v6-review-history.md`.

| Round | Version judged | Coverage (C1–C98, S1) | Simplicity | Elegance |
| --- | --- | --- | --- | --- |
| 1 | v4 | FAIL, narrowly | 6 / 10 | 6 / 10 |
| 2 | v5 | PASS | 7 / 10 | 7 / 10 |
| 3 | v6, this document | PASS | 8 / 10 | 8 / 10 |

**Round 1.** The fourth version cut eighteen tables to ten. The critic showed that its budget was
flattering (about 30 nouns where 19 were claimed, seven tags where five were), that merging the trace
into a table of "measurements" had lost three behaviours of a trace from the text, that flags on an
event had become unwritable without being listed, and that the rule for tables did not produce the
tables.

**Round 2.** The fifth version closed both losses, counted by the critic's method, removed four tags,
a flag, a rule and three refusals, renamed *variable* to *pin*, and added the table of where things go.
The critic found no blocker, confirmed that no smaller vocabulary exists, and asked for a presentation
on two levels, a rule for tables that does not claim to decide what it does not, and a diagnosis that
agrees with the design.

**Round 3.** This version presents nine kinds and four catalogues, states the twelve contracts and the
three merges made by judgement, corrects the diagnosis, puts the derived kind of a function to the user
as a decision, separates the step bound from the call environment, and closes the small gaps of the
normative text. It is shorter than its predecessor.
