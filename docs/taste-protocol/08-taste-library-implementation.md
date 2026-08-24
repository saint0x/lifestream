# Taste Library Implementation

Implementation should make taste usable for creators: clear enough to guide decisions, flexible enough to preserve voice, and practical enough to improve real productions.

## 56. Positive Reference Library

For every major creative principle, provide examples from:

- films
- documentaries
- television
- commercials
- fashion films
- music videos
- YouTube creators
- photographers
- directors
- cinematographers
- editors

For each reference:

1. What specifically is excellent?
2. At what timestamp or scene can it be observed?
3. What principle should be extracted?
4. What should not be copied from it?

---

## 57. Negative Reference Library

Create the inverse archive as well.

For each example:

1. What looks wrong?
2. Why does it look wrong?
3. Which decision caused the failure?
4. How would you repair it?
5. What rule should the system learn from the failure?

This section is extremely important.

A taste system needs to understand not only:

> Do this.

but:

> Never do this unless these specific conditions apply.

---

## 58. Strength Of Rules

Every extracted rule should eventually receive a classification:

### A: Hard Rule

Almost never violate.

### B: Strong Default

Usually follow unless context strongly suggests otherwise.

### C: Style Preference

A Vanta preference, but not inherently superior.

### D: Situational Tool

Useful only under specific circumstances.

### E: Anti-Rule

Frequently cited conventional wisdom that should intentionally be ignored.

For every answer in this questionnaire, indicate its appropriate classification.

---

## 59. When To Break The Rule

For every important principle, answer:

1. What is the rule?
2. Why does it exist?
3. What failure is it preventing?
4. What circumstances justify breaking it?
5. What signal tells you the exception applies?
6. What new failure becomes possible once the rule is broken?

This is critical because elite taste is rarely a collection of absolute rules.

It is:

> A sophisticated understanding of when each rule matters.

---

## 60. Make Judgment Programmatic

Wherever possible, convert creative intuition into conditional reasoning.

For example:

```text
IF subject is emotionally vulnerable
AND performance is authentic
AND background is nondistracting
THEN favor longer close-up holds.
```

Or:

```text
IF camera movement has no narrative motivation
AND static composition remains visually strong
THEN prefer static framing.
```

Or:

```text
IF dialogue communicates information already visible on screen
THEN consider removing dialogue.
```

For every major section of this questionnaire, produce these kinds of decision rules.

---

## 61. Quantifying Taste Where Possible

Specify numerical ranges whenever judgment can reasonably be expressed numerically.

Examples:

- minimum acceptable focus quality
- maximum camera shake
- target face exposure
- acceptable highlight clipping
- target dialogue loudness
- minimum shot duration
- maximum average shot length for specific sequence types
- maximum subtitle density
- minimum readable title duration
- preferred camera movement speed
- minimum usable resolution
- acceptable stabilization crop
- preferred subject headroom

The system should avoid false precision, but anything genuinely measurable should be measured.

---

## 62. What Should The System Do When Unsure?

1. When should it make an autonomous decision?
2. When should it preserve both versions?
3. When should it ask a human?
4. When should it choose the more conservative option?
5. When should it choose the more experimental option?
6. Which creative choices are reversible?
7. Which choices are high-risk?
8. What confidence threshold should trigger escalation?

---

## 63. Alternative Cuts

When should the system generate multiple versions?

Examples:

- restrained cut
- energetic cut
- emotional cut
- commercial cut
- auteur cut

Questions:

1. Which variables should differ?
2. Which variables should remain fixed?
3. How should the best version be selected?
4. Should different cuts be evaluated against viewer response?

---

## 64. Preventing The Protocol From Becoming Dated

1. Which principles are timeless?
2. Which are trend-dependent?
3. How should current visual trends be incorporated?
4. When should trends intentionally be ignored?
5. How often should reference works be updated?
6. How do you distinguish evolution from degradation of taste?
7. Who has authority to modify core rules?

---

## 65. The Executive Director Review

Before anything can be considered Vanta Ready, the system should answer:

### Story

- Is there a clear reason to watch?
- Does every scene advance something?
- Is the strongest material given enough room?
- Does the piece end at the correct moment?

### Image

- Is every frame intentionally composed?
- Are there distracting technical problems?
- Is visual repetition controlled?
- Does the piece have depth and texture?

### Edit

- Does every cut feel motivated?
- Are there unnecessary moments?
- Does rhythm evolve?
- Is anything over-edited?

### Audio

- Is dialogue intelligible?
- Does ambience feel alive?
- Is music used intentionally?
- Are sonic transitions smooth?

### Emotion

- Does the audience know what to feel without being forced to feel it?
- Are emotional peaks protected?
- Is restraint used where appropriate?

### Identity

- Does the creator still feel like themselves?
- Does the project possess its own visual identity?
- Does it nevertheless meet Vanta's quality bar?

### Taste

Finally ask:

> Does anything about this feel cheap, generic, obvious, derivative, overproduced, underdeveloped, or aesthetically unnecessary?

If yes:

> The work is not complete.

---

## 66. Required Answer Format For The Executive Director

For the strongest version of this exercise, the director should not simply answer each question conversationally.

Each meaningful answer should be captured in the following schema:

### Principle Schema

**Rule:**  
The direct instruction.

**Rationale:**  
Why this produces a better result.

**Applies When:**  
Conditions under which the rule is useful.

**Do Not Apply When:**  
Conditions under which it should be ignored.

**Failure Mode Prevented:**  
What normally goes wrong without this principle.

**Good Reference:**  
Specific film, scene, production, frame, or timestamp.

**Bad Reference:**  
Example demonstrating the opposite failure.

**Severity:**  
Hard Rule / Strong Default / Preference / Situational / Anti-Rule.

**Measurable Variables:**  
Any relevant numerical parameters.

**Machine Decision Form:**  
A concise IF / THEN representation.

**Human Judgment Notes:**  
Anything that cannot yet be reliably formalized.

---

## 67. What V1 Should Produce

Once this questionnaire is fully answered, the output should not remain one giant prose document.

It should be decomposed into a structured Vanta Taste Library containing at minimum:

**Principles:**  
The individual rules of taste.

**Decision Trees:**  
Conditional logic governing when rules apply.

**References:**  
Positive examples.

**Anti-References:**  
Examples of failure.

**Thresholds:**  
Measurable technical standards.

**Genre Profiles:**  
Taste differences by content category.

**Emotion Profiles:**  
Taste differences by intended audience state.

**Shot Grammar:**  
Rules governing cinematography.

**Edit Grammar:**  
Rules governing sequencing and pacing.

**Audio Grammar:**  
Rules governing dialogue, music, ambience, and sound design.

**Exception Library:**  
Documented circumstances in which rules should be broken.

**Quality Gates:**  
The standards a production must satisfy before publication.

The ultimate objective is that an intelligent production system should be able to consult this archive and reason:

> Given what is happening in this footage, what would an elite Vanta executive director do next?

That is the actual purpose of the protocol.
