# Can we copy ProPresenter or FreeShow? What is allowed? (plain answer)

*I am not a lawyer. This is the general picture so you can ask a solicitor the right questions. For a product you will sell, pay for an hour with an IP/software solicitor (UK) before launch.*

## The short answer
**No to the plan "give Claude both programs, copy their code, then have a new chat rewrite it".** That is copying with an extra step, not a clean room. I will not take apart, copy or paraphrase ProPresenter's code or files, and you should not upload its installer, app bundle or decompiled files to anyone.

**Yes to the better plan:** build from *what the programs do* (ideas, behaviour, features), written down in our own words, and write our own code. That is legal, it is what every competitor did, and it is what we are already doing. It is also faster.

## Why the "copy then rewrite in a new chat" idea does not work (like I'm 9)
Imagine copying a classmate's homework, then asking a friend to "say it in different words". The teacher can still tell, because the *answers and the way they are worked out* are the same. A different chat is just a different friend. A real "clean room" means the person writing the new code has **never seen** the old code, and only gets a description of what it should *do*. If the same chat (or a chat I summarised) saw the code, the description can leak the code's shape. Courts look at whether the new work was *derived from* the old one, not at how many chats it passed through.

## The three cases

| What you copy | Is it allowed? | What happens if you do it anyway |
|---|---|---|
| **ProPresenter** (Renewed Vision; closed, commercial) | **No.** Code, files, images, icons, layouts and names are protected. Its licence (EULA) normally forbids taking it apart. | Takedown notices, app-store/payment-provider removal, a claim for damages. It also makes your company hard to sell or invest in. |
| **FreeShow** (open source, **GPL-3.0**) | **Only on its terms.** You may copy its code, but then *your whole program* must also be GPL-3.0 and you must give every customer the source. You can still charge money, but you cannot keep your code secret. | Break the licence and you lose the right to use the code at all, and can be sued by the authors. |
| **Ideas, features, how it behaves** ("it can output to screen 2", "it has a countdown") | **Yes.** Copyright protects the way something is written, not what it does. In the UK/EU this is explicit for software functionality (the *SAS v World Programming* case); in the US *Google v Oracle* and *Lotus v Borland* point the same way. | Nothing, as long as you write your own code and your own screens. |

Also keep away from: their **names and logos** ("ProPresenter", "FreeShow"), their **icons and artwork**, a screen layout that is a near copy (trade dress), and any **patented** feature (check before launch; I have not searched patents).

## What I CAN safely use (and what you can give me)
* **Allowed to give me:** your own screenshots of how a program *looks and behaves* when you use it; their **public manuals and help pages**; your own written notes ("when I press X, Y happens"); song/Bible/sermon files you made.
* **Please do not give me:** installers, `.app`/`.exe` files, decompiled code, licence keys, or ProPresenter's own file formats that you would have to take apart to understand.
* **FreeShow source code:** it is public, but if we ever want a *closed, sold* product, **do not read or copy it**. Reading it and then writing something very similar is exactly how accidental copying happens. Use its public *docs* and the running program instead. (If you ever *want* to build on it, the honest route is to fork it and stay GPL-3.0, or contribute upstream.)
* **Libraries with permissive licences (MIT / Apache-2.0 / BSD)** are fine to use in a sold product. Keep their notices. Avoid GPL/AGPL libraries inside our own closed code.

## The options and what happens (best first)

1. **Spec-first, own code (recommended).** We write a *behaviour spec* from public docs, your screenshots and your notes ("Output screen picker: lists screens, remembers the choice, falls back to screen 1"). I implement from the spec. **Outcome:** clean, original code, quickest path, fully sellable. *This is what v0.814.0 did for screen output, colours, history and media.*
2. **Proper clean room (only if a lawyer asks for it).** Two separate people/chats: A studies the other program and writes behaviour-only specs (no code, no strings, no layouts); B never sees A's materials except the specs. Keep a log. **Outcome:** strongest legal position, slower, and you need real separation. An AI chat is not a legal firewall, so it is weaker than two humans.
3. **Fork FreeShow and sell support or hosting (GPL).** **Outcome:** legal and quick, but not secret: every customer can get the source and share it. A fine business, but a different one (service, not secret software).
4. **Copy and reword (what you suggested).** **Outcome:** looks fast, but the risk is highest and the damage lands later, exactly when you have customers and money.
5. **Use ideas from the roadmap only.** **Outcome:** safe, but you must still do the building (that is what we are doing).

## Checklist before you sell
1. Name, logo and icons are our own. 2. Every library listed with its licence (a `THIRD-PARTY.md`). 3. No code, text or images copied from other programs. 4. Bible texts only with a licence (see `LICENSING-AND-PRICING.md`). 5. A terms-of-service and privacy policy (you will store emails and payments). 6. UK: register as a company or sole trader, think about trade-mark search for the product name, and ask a solicitor to review the above once.
