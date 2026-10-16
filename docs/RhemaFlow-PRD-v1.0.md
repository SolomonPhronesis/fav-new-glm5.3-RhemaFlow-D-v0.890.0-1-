# **RhemaFlow**

## **Product Requirements Document — Version 1.0**

### **1\. Product Overview**

**RhemaFlow** is a smart church presentation and sermon intelligence platform designed to understand a preacher's spoken message in real time.

RhemaFlow listens during a church service, recognizes spoken Scripture references and recognizable Scripture quotations, retrieves the appropriate Bible passage, and automatically presents it to the congregation.

At the same time, RhemaFlow records the sermon, creates a timestamped transcription, identifies scriptures referenced during the service, and builds a searchable sermon archive.

The product is designed around one central principle:

> **The preacher speaks naturally. RhemaFlow understands and presents.**

RhemaFlow should automate as much of the live presentation experience as possible while ensuring the church's media team always retains meaningful control.

---

## **2\. Problem**

Church media teams often have to listen carefully to a preacher, identify Bible references, search for the correct passage, select the appropriate translation, prepare the presentation, and display the result at the right moment.

This creates several problems:

* Scripture can appear late because someone has to find it manually.  
* The media operator must divide attention between the preacher and presentation system.  
* A preacher can move to a new passage faster than the media operator can prepare it.  
* Spontaneous Scripture references can be missed.  
* Recordings do not automatically contain a structured index of the Scriptures used.  
* Sermon recordings can become difficult to search or revisit later.  
* Churches may already have presentation workflows they do not want to replace.

RhemaFlow addresses these problems by allowing the presentation system to understand the sermon rather than simply wait for manual instructions.

---

## **3\. Product Vision**

RhemaFlow should become the church's intelligent live presentation assistant.

It should eventually understand:

* What Scripture the preacher is referring to.  
* What translation the preacher wants.  
* When a Scripture should appear.  
* When the preacher refers back to an earlier passage.  
* What important sermon statements may be useful as presentation content.  
* What happened during the service.  
* Where important moments occurred in the recording.

The system should remain helpful without becoming intrusive.

---

## **4\. Target Users**

### **Primary Users**

**Preachers and speakers**

They should be able to preach naturally without learning special commands or changing their speaking style.

**Church media and AV operators**

They should have visibility and control over what RhemaFlow detects and presents.

### **Secondary Users**

**Church administrators**

They manage church settings, users, services and archive access.

**Church members and viewers**

They may consume shared sermon recordings and searchable sermon content.

**Ministries and larger church organizations**

They may operate multiple services, teams or locations.

---

## **5\. Core Product Promise**

RhemaFlow should provide:

> **Automatic presentation when confidence is high, human control when judgment is needed.**

The preacher should not have to operate the software.

The media operator should not have to manually perform tasks that RhemaFlow can safely perform itself.

---

# **6\. Core Features**

## **6.1 Voice-Activated Scripture Trigger**

RhemaFlow continuously interprets the selected live service audio and identifies Scripture references.

It should understand natural expressions such as:

* “John 3:16.”  
* “John chapter 3 verse 16.”  
* “John chapter 3 verses 16 through 18.”  
* “First Corinthians chapter 13.”  
* “Let's go back to verse 7.”  
* “Turn with me to Psalm 23.”

When a valid Scripture reference is identified with sufficient confidence, RhemaFlow should present it automatically.

The operator must be able to override or cancel the result.

---

## **6.2 Scripture Quotation Recognition**

RhemaFlow should also recognize recognizable quotations of Scripture even when the preacher does not state the reference.

For example:

> “For God so loved the world…”

RhemaFlow should be able to identify the likely Scripture and make it available for presentation.

The operator must retain the ability to approve, modify or reject uncertain detections.

---

## **6.3 Natural Context Understanding**

RhemaFlow should understand references in context rather than treating every statement independently.

Examples include:

> “Let's go back to that verse.”

> “Now look at verse 7.”

> “Compare that with Romans 8.”

Context should help RhemaFlow understand what the preacher is referring to during the current service.

---

## **6.4 Translation Management**

RhemaFlow should support a church-selected default translation.

It should also recognize when the preacher explicitly asks for another translation.

Example:

> “Let's read John 3:16.”

RhemaFlow uses the church's default.

Then:

> “Now let's read it in the NIV.”

RhemaFlow presents the requested translation where the church has access to it.

The product should support public-domain Bible translations directly while providing a pathway for churches to use appropriately licensed translations.

---

## **6.5 Confidence and Operator Review**

When RhemaFlow is uncertain, it should not silently make an unreliable decision.

Instead, the operator should receive a useful suggestion.

Example:

**Possible Scripture detected**

John 3:15  
John 3:16

**Approve · Correct · Dismiss**

The system should make uncertainty visible without making the operator manually verify every straightforward Scripture reference.

---

## **6.6 Smart Scripture Presentation**

When Scripture is displayed, RhemaFlow should automatically create a polished presentation containing the appropriate information, including:

* Scripture text  
* Bible reference  
* Translation  
* Appropriate formatting

The layout should adapt to the amount of content.

Short passages should remain prominent and readable.

Longer passages should remain easy to follow.

---

## **6.7 Church Branding and Presentation Styles**

RhemaFlow should provide a polished default presentation style.

Churches should also be able to customize their presentation appearance.

Possible customization includes:

* Church branding  
* Logo  
* Visual style  
* Fonts  
* Colours  
* Backgrounds  
* Presentation layouts

Churches should be able to save multiple styles for different services or events.

---

# **7\. Sermon Intelligence**

## **7.1 Live Transcription**

RhemaFlow should create a live transcription of the sermon.

The transcript should provide context for the live service and become part of the sermon archive.

---

## **7.2 Scripture Index**

Every detected Scripture reference should become part of the sermon record.

Each entry should contain the relevant time within the service.

Example:

**Sunday Service**

10:42 — John 3:16  
17:15 — Romans 8:28  
31:07 — Psalm 23:1–6

Selecting an entry should take the user to that point in the sermon.

---

## **7.3 Sermon Timeline**

The service should have a chronological timeline containing meaningful detected events.

Examples:

* Scripture references  
* Translation changes  
* Suggested presentation content  
* Important sermon moments

This timeline becomes part of the completed sermon archive.

---

## **7.4 Suggested Sermon Content**

RhemaFlow should identify potentially useful presentation moments.

For example:

> “There are three things I want you to remember: Faith, Obedience and Perseverance.”

RhemaFlow could suggest:

**Faith**  
**Obedience**  
**Perseverance**

The operator can:

* Approve  
* Edit  
* Reject

This is the initial product behaviour.

Long-term, churches may allow selected types of content to be generated automatically.

---

# **8\. Live Operator Command Centre**

The media operator should have one central workspace showing the state of the service.

The command centre should provide visibility into:

* Live transcription  
* Current detected Scripture  
* Current presentation  
* Translation  
* Uncertain detections  
* Suggested content  
* Scripture history  
* Service timeline  
* Recording state  
* Presentation controls

The goal is not to give the operator more work.

The goal is to give the operator **one place to understand what RhemaFlow is doing**.

---

# **9\. Service Modes**

RhemaFlow should support different listening modes.

Examples:

**Full Service Mode**

RhemaFlow remains available throughout the service.

**Sermon Mode**

RhemaFlow focuses on the preaching/teaching portion.

**Bible Reading Mode**

RhemaFlow focuses on Scripture reading.

**Custom Service Mode**

The church chooses when RhemaFlow should actively respond.

This prevents unwanted triggers during unrelated parts of the service.

---

# **10\. Audio Source Management**

RhemaFlow should support different church audio workflows.

The system should be able to work with an appropriate preacher/audio source while allowing the operator to change or select the source when necessary.

The operator should never be locked into a single workflow.

---

# **11\. Recording**

RhemaFlow should create a high-quality sermon recording and associate it with the generated sermon record.

The recording should remain connected to:

* Transcript  
* Scripture index  
* Timeline  
* Presentation events  
* Sermon metadata

The church should be able to maintain both local and cloud copies according to its preferences.

---

# **12\. Sermon Archive**

Every completed service should become a searchable sermon record.

A sermon record can contain:

* Sermon title  
* Date  
* Speaker  
* Service  
* Transcript  
* Audio/video recording  
* Scripture index  
* Translation information  
* Timeline  
* Presentation events  
* Church-defined categories or tags

The archive should support searching by meaningful information such as:

* Scripture  
* Speaker  
* Date  
* Sermon  
* Series  
* Topic  
* Tags

---

# **13\. Post-Service Editing**

Church users should be able to correct mistakes after the service.

Examples:

* Correct a transcription error.  
* Correct an incorrectly identified Bible reference.  
* Rename a sermon.  
* Edit sermon metadata.  
* Correct the displayed translation information.  
* Adjust archive categories.

Corrections should not destroy the underlying original record.

---

# **14\. Sharing**

Churches should be able to share completed sermons.

A shared sermon experience should provide useful access to:

* Sermon information  
* Recording  
* Transcript  
* Scripture index  
* Timestamped sermon moments

Churches should have control over what is publicly shared.

Downloading should remain available where permitted by the church.

---

# **15\. Offline and Online Operation**

RhemaFlow should continue providing essential live functionality even when internet access is interrupted.

The church should not lose the basic ability to conduct a service simply because cloud connectivity is temporarily unavailable.

When connectivity is restored, appropriate cloud information should synchronize.

This is especially important for live worship environments where service continuity matters.

---

# **16\. Church Accounts and Roles**

RhemaFlow should support different user roles.

Examples include:

**Church Administrator**

Manages the church, settings, users and archive.

**Media Operator**

Controls live services and presentation.

**Speaker/Preacher**

Can view relevant sermon information and service content.

**Archive Viewer**

Can access permitted recordings and sermon information.

Churches should be able to control access according to their needs.

---

# **17\. Multi-Service and Multi-Location Support**

A church should be able to organize multiple services and locations under its account.

Examples:

* Sunday Morning  
* Sunday Evening  
* Wednesday Bible Study  
* Youth Service  
* Main Campus  
* North Campus  
* Online Service

Each service should retain its own sermon record.

---

# **18\. Translation Expansion**

The initial product should prioritize excellent English-language support.

RhemaFlow should then expand to additional spoken languages and Bible-language combinations.

The product should be designed with multilingual growth in mind without allowing that broader ambition to weaken the initial experience.

---

# **19\. Existing Presentation Software**

RhemaFlow should not require every church to abandon its existing presentation workflow.

It should be able to operate as:

**A complete presentation system**

or

**An intelligent companion to an existing presentation system.**

This is particularly important for churches that have already invested time and resources into their current workflows.

---

# **20\. Service Preparation**

RhemaFlow should support both prepared and spontaneous services.

Before a service, a church may optionally prepare:

* Service information  
* Speaker  
* Sermon title  
* Expected Scriptures  
* Preferred translation  
* Presentation style  
* Other relevant information

During the service, RhemaFlow should continue responding to new or unexpected material.

Preparation should make RhemaFlow better informed, not mandatory.

---

# **21\. Reusable Service Templates**

A church should be able to save common service arrangements.

Example:

**Sunday Morning Template**

* Main presentation style  
* Default translation  
* Main service settings  
* Recording preferences  
* Operator permissions

The operator can start with the template and adjust it for the current service.

---

# **22\. Privacy and Data Ownership**

Churches should have clear control over their sermon information.

The product should allow churches to manage:

* What is recorded  
* How long content is retained  
* Who can access it  
* What is shared  
* What is deleted

The church should retain control over its sermon archive.

Privacy and trust should be treated as core product requirements rather than secondary settings.

---

# **23\. Core User Journey**

## **Before the service**

The media operator opens RhemaFlow.

They select or start a service.

They can use an existing service template or prepare the current service.

The church's chosen presentation style and default translation are loaded.

---

## **During the service**

The preacher speaks naturally.

RhemaFlow listens.

The preacher says:

> “Let's turn to John chapter 3, verse 16.”

RhemaFlow identifies the reference.

The Scripture appears on screen.

The operator can override it if necessary.

The preacher later says:

> “For God so loved the world…”

RhemaFlow recognizes the likely Scripture quotation.

The operator can approve or correct it.

The service continues without requiring the preacher to interact with the software.

---

## **During an uncertain moment**

RhemaFlow identifies an uncertain reference.

The operator receives a suggestion.

The operator can approve, correct or dismiss it.

The service continues.

---

## **During a sermon point**

The preacher says:

> “There are three things I want you to remember…”

RhemaFlow identifies potentially useful presentation content.

A suggestion appears for the operator.

The operator edits or approves it.

---

## **After the service**

The recording is saved.

The transcript is available.

The Scripture index is automatically created.

The sermon timeline is available.

The operator can make corrections.

The sermon becomes part of the church archive.

The church can share the sermon if desired.

---

# **24\. MVP Definition**

The first version of RhemaFlow should concentrate on the central promise rather than attempting to become every type of church software.

### **Essential first-release capabilities**

**Live Scripture understanding**

Natural spoken Scripture references should be recognized.

**Automatic Scripture presentation**

Detected Scripture should appear automatically.

**Operator override**

Operators should be able to correct, approve or dismiss detections.

**Translation selection**

A church default should exist, with recognition of explicitly requested translations.

**Scripture quotation recognition**

RhemaFlow should recognize common quoted Scripture even without an explicitly spoken reference.

**Live transcription**

The sermon should be transcribed.

**Scripture timeline**

Every detected Scripture should be linked to its place in the sermon.

**Recording**

The service should be recorded and associated with its sermon record.

**Church presentation customization**

A polished default style should exist, with church customization.

**Operator command centre**

The media team should have one central live dashboard.

**Local and cloud saving**

The church should be able to preserve sermon content according to its chosen workflow.

**Searchable sermon archive**

Completed sermons should remain useful after the live service.

---

# **25\. Later Expansion**

After the core experience is trusted, RhemaFlow can expand into:

* More automatic presentation content generation  
* Greater sermon understanding  
* Additional languages  
* More sophisticated multi-location workflows  
* Deeper presentation integration  
* More advanced sermon discovery  
* Richer sermon sharing experiences  
* More automated service preparation

The key principle should remain unchanged:

> **Automation should grow as trust grows.**

---

# **26\. Non-Goals for the Initial Product**

RhemaFlow should not initially attempt to become:

* A complete replacement for every church media tool.  
* A complete church management system.  
* A worship/music management platform.  
* A general-purpose AI assistant unrelated to church presentation.  
* A generic video editing application.

Its differentiation should remain strongly centered on:

> **Understanding live preaching and turning it into intelligent presentation and sermon records.**

---

# **27\. Product Success Measures**

The most important question is not simply whether RhemaFlow can recognize speech.

The product succeeds when it meaningfully reduces the amount of manual work required from the church media team.

Important measures include:

**Automatic Scripture presentation**

How often does RhemaFlow successfully handle a Scripture reference without requiring manual intervention?

**Useful recognition**

How often are detected references and quotations correct?

**Operator workload**

How much manual interaction is required during a normal sermon?

**Service continuity**

Can the preacher continue speaking naturally without changing their behaviour for RhemaFlow?

**Archive usefulness**

Can a church easily find a specific Scripture or moment from a previous sermon?

**Church adoption**

Do churches continue using RhemaFlow after their initial trial?

**Post-service value**

Do churches actually revisit, search and share their archived sermons?

---

# **28\. Product Principles**

RhemaFlow should be guided by these principles:

### **1\. Preacher first**

The preacher should speak naturally.

### **2\. Automation first**

RhemaFlow should perform repetitive presentation work automatically.

### **3\. Human control always**

The media operator should be able to intervene.

### **4\. Understand context**

RhemaFlow should understand what the preacher means, not merely detect keywords.

### **5\. Beautiful by default**

Automatically generated presentation content should look professional.

### **6\. Useful after the service**

The sermon should become a valuable searchable record.

### **7\. Church-controlled**

The church should control presentation, access, sharing and its sermon archive.

### **8\. Start focused, expand intelligently**

The product should win through its Scripture and sermon intelligence before expanding into a much broader church platform.

---

# **29\. One-Sentence Product Definition**

> **RhemaFlow is an intelligent church presentation and sermon platform that listens to preachers in real time, understands Scripture and sermon content, automatically presents relevant information, and turns every service into a searchable, timestamped sermon record.**

# **30\. Product Tagline**

> **RhemaFlow — The preacher speaks. RhemaFlow understands.**

