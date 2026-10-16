# Bible licences, competitor prices, what to charge, and how accounts work

*Not legal or financial advice. Prices change; I checked the web on 5 Oct 2026 and say where a figure is from. Check with each publisher before you rely on it.*

## 1. Why NIV, NKJV, AMP, ESV, NLT cost money (like I'm 9)
A Bible translation is like a song: someone **wrote it**, and the writers (the publisher) own it. Old ones (KJV 1611, ASV 1901) are free to everyone. New ones belong to a company that asks for money or permission, because translating takes years and they pay for it from licences. If our program shows their words to many churches, we are *copying and showing* their work, so we need a **licence**.

## 2. What I found (and what I could not find)
| Version | Owner | What the sources say | Our best route |
|---|---|---|---|
| **NIV** | Biblica (UK: Hodder & Stoughton) | API.Bible's own docs say NIV is **not** available through their quick commercial licence; you contact Biblica. Biblica licenses **registered companies**, fees/royalties are **case by case**, and they are cautious about products still in development. For plain *church-service use* (showing verses in a service) their guidance has been permissive, but **selling software that contains NIV text is different**. | Get the text through a licensed platform *with the church's own access*, or approach Biblica once we have a company and paying users. |
| **NKJV** | Thomas Nelson / HarperCollins Christian | Sold to church-software makers as a **per-user unlock** (an older ProPresenter store page listed NKJV and AMPC at roughly **$15 each** one-time). I could not find a published developer fee. | Ask HarperCollins Christian Publishing licensing; or let churches bring their own licensed copy. |
| **AMP / AMPC** | The Lockman Foundation | Same older store listing (~$15 one-time per church for AMPC). Lockman has a permissions page for software. No public developer fee found. | Ask Lockman; or bring-your-own-licence. |
| **ESV** | Crossway | Has its own API with a free non-commercial key; commercial use needs permission. Not on API.Bible. | Crossway's API/permissions. |
| **NLT** | Tyndale | Permission-based for software. | Tyndale licensing. |
| **KJV, ASV, YLT, Darby, Webster, Rotherham, AKJV, BBE** | public domain | Free. **Already bundled.** | Done. |

**Your "is it $10k a year?" question:** I found **no fixed price** for any of them. Fees are **negotiated** and depend on how many users you have and how much text you show; for aggregators the fee is by monthly active users. The only hard figure I found: **API.Bible's commercial plan starts around $29/month plus per-translation licence fees**, and cached text must be refreshed about every 30 days. So "$10k" is possible for a big deal and wrong for a small start. **Do not guess: email each publisher with: who you are, a one-paragraph product description, expected number of churches, which text, how much is shown at once.**

**Two ways to avoid paying everything up front**
1. **Bring your own licence / key:** the church buys the Bible (or its own API key) and enters it in the app. We never store or resell the text. (This is how ProPresenter's store worked: *"a site licence for the program is not a site licence for a Bible"*.) Cheapest and fastest for us.
2. **Aggregator licence (API.Bible, YouVersion platform, Digital Bible Platform):** one agreement, many versions, they handle publishers; you pay platform + per-user fees. Good once we have users.

## 3. The competition (checked 5 Oct 2026)
* **ProPresenter** (Renewed Vision): new users are subscription-only; third-party sites report about **$29/month or $289/year** per person (campus and 4+ seat tiers are cheaper per seat). Your "£250+ a year" guess is about right.
* **FreeShow:** free, open source (GPL-3.0), no support contract. Great and free, so our reason to exist is **voice, speed and polish**, not "a presenter".
* **SmartVerses:** AI that listens and finds verses and plugs into ProPresenter; free plan; about **$12.60/month (billed yearly)** Standard and **$27/month** Pro. This is the closest rival to what you heard about.
* **Pewbeam** (offline-first), **CiteVerse**, **LiveVerse** (Chrome extension), **Church Presenter** (free): smaller players.

## 4. What to charge (my reasoning, then your call)
Your difference: **offline voice detection, cheap, works with weak Wi-Fi, all-in-one (verses + slides + media + stream output)**.

| Plan | Price | For | Why |
|---|---|---|---|
| **Free** | £0 | Any church | KJV + 8 public-domain Bibles, search, history, browser, one screen. This feeds word of mouth and costs us nothing per user. |
| **Standard** | **£10 / month** (or £99 / year) | Small church | Voice detection, 2nd-screen output, transcripts, sermon plan, phone remote. |
| **Pro** | **£20 / month** (or £199 / year) | Church with a media team | Everything: media, slides, OBS output, more devices, priority support. |
| **Add-on Bibles** | £2-£5 / month each, or bring your own key | Churches that want NKJV/AMP etc. | Covers the publisher fee; only offered once the licence exists. |

*Why not cheaper:* under ~£8 you cannot pay for support. *Why not dearer at first:* ProPresenter is about £215-£230/year for everything; to win we should look clearly cheaper for what the church actually uses. £10 and £20 per month are sensible starting points; **test with 10 churches** (offer 3 months at half price) before you fix the prices. Raise prices once support load and licence costs are known.

## 5. Accounts and the internet (how to do it safely)
**Stack:** Supabase (login + database) + Stripe (card payments) + a small server function.
1. Church signs up (email + password or Google). Supabase stores the account.
2. Stripe Checkout takes the subscription. A Stripe **webhook** tells a Supabase function "this account is paid until DATE" and writes it to a `subscriptions` table.
3. The app signs in and asks the server for a **signed ticket** ("account X, Pro, paid until DATE, up to 3 devices"). The ticket is signed with a secret that only the server has; the app checks it with the matching *public* key, so nobody can edit the dates.
4. **How often must it go online?** Options:

| Check-in | Good | Bad |
|---|---|---|
| every 24 h | hardest to cheat | breaks churches with poor or no Wi-Fi; Sunday service fails if the Saturday check failed |
| every 72 h | still strict | the same problem on a bad week |
| **every 7 days + 14 days grace (my pick)** | a church that meets weekly gets a fresh ticket whenever Wi-Fi works once a week; two missed weeks still work | a determined cheater gets about 3 weeks (small cost) |
| monthly | friendliest | easier to share one login |

   **Built and tested already:** `crates/rf-app/src/license.rs` has these rules (weekly check-in, 14 days of grace, 5 days of payment grace, notices the clock being wound back, **never cuts a service in progress**, and falls back to a **free mode** instead of a locked screen). Not yet connected to a server; that is Stage 3 in the roadmap.
5. Limit devices (2-3 per account) and show "sign out other devices".
6. Store **only** what you need (email, plan, device ids). Do not store card numbers (Stripe does). Write a privacy policy.
7. Never let a payment problem blank the screen on Sunday.

## 6. Who to email, in order
1. Lockman (AMP/AMPC) 2. HarperCollins Christian (NKJV) 3. Crossway (ESV) 4. Tyndale (NLT) 5. Biblica/Hodder (NIV, after you have a company and users) 6. API.Bible (ask for a commercial quote for your user estimate). Keep every reply; it is part of your company's paperwork.

## 7. Update 11 Oct 2026: The Message and NIV (from the publishers' own pages)
* **The Message** (NavPress, handled by Tyndale): text may be quoted free up to 500 verses if that is under 25% of the work and not a whole book. A whole-Bible app is outside that, so write to **permission@tyndale.com** (say what the product is, how many churches, how much text shows at once). Permission covers only the use you asked for and cannot be passed on. No public price.
* **NIV** (Biblica; UK: Hodder): same 500-verse / 25% rule. Biblica says it generally does not license products still in development, wants proof of a legal entity, and takes about 10 business days to review plus about 10 to write a letter. API.Bible says it cannot sell NIV commercial rights; go to Biblica (biblica.com/permissions). A church showing NIV in its own services is covered by the notice rules; a company selling software that contains the NIV is not.
* **NKJV, AMP/AMPC**: no public developer price found; ask HarperCollins Christian and the Lockman Foundation (or let churches bring their own licence).
