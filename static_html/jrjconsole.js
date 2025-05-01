// (C) Copyright 2025 Joseph R. Jones - https://jrj.org - MIT License

(() => {
  const term = document.getElementById("terminal");
  const charDelay = 3;
  // Generate a random IPv4 address for fun hacker alerts
  function randomIP() {
    return Array(4).fill(0)
      .map(() => Math.floor(Math.random() * 256))
      .join('.');
  }

  // Client-side list of shortcodes for autocomplete
  let SHORTCODES = [];
  // Load shortcodes file (one code per line)
  fetch("shortcodes.txt")
    .then(res => res.text())
    .then(text => {
      SHORTCODES = text.split(/\r?\n/).filter(line => line);
    })
    .catch(() => {
      // ignore if not available
    });
  const COMMANDS = {
    help: () => printLines([
      "Available commands:",
      "  <span class='clickable' data-command='what'>what</span>             – info about this service",
      "  <span class='clickable' data-command='about'>about</span>            – about JRJ",
      "  <span class='clickable' data-command='contact'>contact</span>          – how to reach JRJ",
      "  <span class='clickable' data-command='wally'>wally</span>            - who's a good boy?",
      "  <span class='clickable' data-command='clear'>clear</span>            – clear the screen",
      "  <span class='clickable' data-command='help'>help</span>             – show this list",
      "  <span class='clickable' data-command='open [shortcode]'>open [shortcode]</span> – open url in new tab",
      " ",
    ]),
    what: () => printLines([
      "JRJ's personal link shortener",
      "Built in Rust <img src='./ferris.png' style='height: 1em;' class='clickable' data-command='rusty'>",
      "Source: https://github.com/jrjones/redirective",
      "Type 'help' to explore.",
      " ",
    ].map(linkify)),
    about: () => printLines([
      "Joseph R. Jones (JRJ)",
      "Scruffy-looking nerd herder",
      "and world's foremost expert",
      "in self-proclaimed thought leadership",
      "https://jrj.org",
      " ",
    ].map(linkify)),
    contact: () => printLines([
      "This is kind of one of those \"if you know, you know\" situations.",
      "If I wanted you to contact me you would know how to contact me.",
      "",
      "That said, JRJ...",
      " - Is on twitter/X as @jrj (but doesn’t really post anymore)",
      " - Used to blog at https://blog.jrj.org (but the site is archived)",
      " - Is on LinkedIn -> https://jrj.io/in (but doesn’t accept unknown connections)",
      " ",
    ].map(linkify)),
    wally: () => printLines([
      "Who's a good boy?",
      "<a href=wally.png><img src=wally.png width=200></a>",
      "Wally's a good boy!",
      " ",
    ].map(linkify)),
    rusty: () => printLines([
      "cargo build --release",
      "<span class='yellowbold'>Compiling</span> metrics.rs",
      "<span class='yellowbold'>Compiling</span> main.rs",
      "<span class='yellowbold'>Compiling</span> http.rs",
      "<span class='yellowbold'>Compiling</span> git_sync.rs",
      "<span class='yellowbold'>Compiling</span> errors.rs",
      "<span class='yellowbold'>Compiling</span> config.rs",
      "<span class='yellowbold'>Compiling</span> cache.rs",
      "<span class='yellowbold'>Finished</span> `release` target(s) in 1.23s",
    ]),
    clear: () => { term.innerHTML = "jrj.io redirective shell v1.0<br>Type 'help' for a list of commands."; newPrompt(); },
    open: (args) => {
      // If no argument, list available shortcodes
      if (!args[0]) {
        if (SHORTCODES.length > 0) {
          printLines(SHORTCODES);
        } else {
          printLines(["(no shortcodes available)"]);
        }
        return;
      }
      const code = args[0];
      // If it's a known shortcode, open via redirect endpoint
      if (SHORTCODES.includes(code)) {
        window.open("/" + code, "_blank");
      } else {
        // fallback: treat as URL
        window.open(code, "_blank");
      }
      print("Opening " + args[0]);
      newPrompt();
    },

    ls: (args) => {
      const files = ["ascii.txt", "ferris.png", "index.html", "jrjconsole.js", "styles.css", "wally.png"];
      const links = files.map(f => `<a href="${f}" target="_blank">${f}</a>`);
      printLines([links.join("  "), " "]);  
    },
    pwd: () => printLines([
      "/home/jrj",
      " "
    ]),
    cd: (args) => {
      if (args[0]) {
        printLines([`bash: cd: ${args[0]}: No such file or directory`]);
      } else {
        newPrompt();
      }
    },
    echo: (args) => print(args.join(" "), newPrompt),
    whoami: () => printLines([
      "jrj",
      " "
    ]),
    date: () => printLines([
      new Date().toString(),
      " "
    ]),
    uname: () => printLines([
      "Linux",
      " "
    ]),
    id: () => printLines([
      "uid=1000(jrj) gid=1000(jrj) groups=1000(jrj)",
      " "
    ]),
    mkdir: (args) => printLines([]),
    rmdir: (args) => printLines([]),
    rm: (args) => printLines([]),
    touch: (args) => printLines([]),
    // Hidden fun hacker-detection commands
    su: () => {
      printLinesThen([
        "<span style='color:red'>Security Alert: Unauthorized access detected!</span>",
        `<span style='color:red'>IP address: ${randomIP()}</span>`,
        " "
      ], () => {
        setTimeout(() => {
          fetch("ascii.txt")
            .then(res => res.text())
            .then(text => {
              const lines = text.split(/\\r?\\n/);
              printLines(lines);
            });
        }, 1000);
      });
    },
    sudo: (args) => {
      printLinesThen([
        "<span style='color:red'>Security Alert: Unauthorized sudo access detected!</span>",
        `<span style='color:red'>IP address: ${randomIP()}</span>`,
        " "
      ], () => {
        setTimeout(() => {
          fetch("ascii.txt")
            .then(res => res.text())
            .then(text => {
              const lines = text.split(/\\r?\\n/);
              printLines(lines);
            });
        }, 1000);
      });
    },
  };

  let inputSpan = null;
  let cursorSpan = null;
  // Autocomplete state
  let suggestionDiv = null;
  let ghostSpan = null;
  let currentSuggestions = [];
  let selectedSuggestionIndex = 0;

  function scrollBottom() {
    // Scroll to bottom after content updates (next frame) to ensure new lines are shown
    requestAnimationFrame(() => {
      term.scrollTop = term.scrollHeight;
    });
  }

  function typeChars(str, cb) {
    let i = 0;
    (function type() {
      if (i < str.length) {
        inputSpan.textContent += str[i++];
        setTimeout(type, charDelay);
      } else if (cb) cb();
    })();
  }

  function linkify(text) {
    return text.replace(/(https?:\/\/[\w\-._~:/?#[\]@!$&'()*+,;=%]+)/g, '<a href="$1" target="_blank">$1</a>');
  }

  function print(line = "", cb) {
    const div = document.createElement("div");
    term.appendChild(div);
    scrollBottom(); // bring new line into view before typing

    let i = 0;
    (function type() {
      if (i < line.length) {
        div.innerHTML = line.slice(0, i + 1); // update content progressively
        scrollBottom(); // keep scrolling as characters are added
        i++;
        setTimeout(type, charDelay);
      } else {
        scrollBottom(); // ensure fully typed line is visible
        if (cb) cb();
      }
    })();
  }

  function printLines(lines) {
    let i = 0;
    (function printNext() {
      if (i < lines.length) {
        print(lines[i++], () => setTimeout(printNext, 100)); // Wait for the current line to finish before starting the next
      } else {
        newPrompt();
      }
    })();
    scrollBottom(); // Ensure the viewport scrolls to the bottom after all lines are printed
  }
  // Variant of printLines that invokes a callback instead of newPrompt at end
  function printLinesThen(lines, cb) {
    let i = 0;
    (function printNext() {
      if (i < lines.length) {
        print(lines[i++], () => setTimeout(printNext, 100));
      } else if (cb) {
        cb();
      }
    })();
    scrollBottom(); // Ensure the viewport scrolls to the bottom
  }
  // Autocomplete helpers
  function clearSuggestions() {
    currentSuggestions = [];
    if (suggestionDiv) suggestionDiv.innerHTML = '';
    if (ghostSpan) ghostSpan.textContent = '';
  }

  function updateSuggestionDisplay() {
    if (!suggestionDiv || !ghostSpan) return;
    // Clear previous suggestions
    suggestionDiv.innerHTML = '';
    const full = inputSpan.textContent;
    const idx = full.indexOf(' ');
    // Only autocomplete arguments after the command
    if (idx <= 0 || currentSuggestions.length === 0) {
      ghostSpan.textContent = '';
      return;
    }
    // Position suggestions under the argument start
    const promptLen = 2; // length of '$ '
    suggestionDiv.style.marginLeft = `${promptLen + idx + 1}ch`;
    // Compute argument prefix and selected suggestion
    const argPrefix = full.slice(idx + 1);
    const sel = currentSuggestions[selectedSuggestionIndex];
    // Inline ghost for the remainder of the suggestion
    if (sel.startsWith(argPrefix)) {
      ghostSpan.textContent = sel.slice(argPrefix.length);
    } else {
      ghostSpan.textContent = '';
    }
    // Show list of up to 5 suggestions, highlighting the selected one
    currentSuggestions.slice(0, 5).forEach((s, i) => {
      const line = document.createElement('div');
      line.textContent = s;
      if (i === selectedSuggestionIndex) {
        // Highlight selection
        line.style.color = '#ffff00';
      }
      suggestionDiv.appendChild(line);
    });
  }

  function applySuggestion(withSpace) {
    if (currentSuggestions.length === 0) return;
    const sel = currentSuggestions[selectedSuggestionIndex];
    // Preserve command prefix (e.g., 'open') and replace argument only
    const text = inputSpan.textContent;
    const idx = text.indexOf(' ');
    let newText;
    if (idx > 0) {
      const cmd = text.slice(0, idx);
      newText = cmd + ' ' + sel + (withSpace ? ' ' : '');
    } else {
      newText = sel + (withSpace ? ' ' : '');
    }
    inputSpan.textContent = newText;
    clearSuggestions();
  }

  function updateAutocomplete() {
    const text = inputSpan.textContent;
    const idx = text.indexOf(' ');
    if (idx > 0 && text.slice(0, idx).toLowerCase() === 'open') {
      const prefix = text.slice(idx + 1);
      currentSuggestions = SHORTCODES.filter(c => c.startsWith(prefix));
      selectedSuggestionIndex = 0;
      updateSuggestionDisplay();
    } else {
      clearSuggestions();
    }
  }

  function runCommand(cmdLine) {
    const parts = cmdLine.trim().split(/\s+/);
    const cmd = parts[0].toLowerCase();
    const args = parts.slice(1);
    if(COMMANDS[cmd]) {
      COMMANDS[cmd](args);
    } else if(cmd) {
      print("Command not found: " + cmd + ". You may not have permission");
      newPrompt();
    }
  }

  // Create a new input prompt with autocomplete container
  function newPrompt() {
    // Reset autocomplete state
    currentSuggestions = [];
    selectedSuggestionIndex = 0;
    ghostSpan = null;
    const wrapper = document.createElement("div");
    wrapper.innerHTML = `<span class="prompt">$ </span><span id="input"></span>`;
    inputSpan = wrapper.querySelector("#input");
    // Create ghost inline suggestion span
    ghostSpan = document.createElement("span");
    ghostSpan.className = "ghost";
    wrapper.appendChild(ghostSpan);
    // Create cursor
    cursorSpan = document.createElement("span");
    cursorSpan.className = "cursor";
    wrapper.appendChild(cursorSpan);
    // Suggestion list container
    suggestionDiv = document.createElement("div");
    // margin-left will be set dynamically in updateSuggestionDisplay
    suggestionDiv.style.whiteSpace = 'pre';
    wrapper.appendChild(suggestionDiv);
    term.appendChild(wrapper);
    scrollBottom();
  }

  function handleKey(e) {
    if (!inputSpan) return;
    const key = e.key;
    // Autocomplete interactions
    if (key === 'Tab') {
      e.preventDefault();
      if (currentSuggestions.length > 0) {
        // Accept suggestion without extra space
        applySuggestion(false);
      }
      return;
    }
    // Navigate suggestions (Arrow keys)
    if (currentSuggestions.length > 0 && (key === 'ArrowDown' || key === 'ArrowUp' || e.keyCode === 40 || e.keyCode === 38)) {
      e.preventDefault();
      if (key === 'ArrowDown' || e.keyCode === 40) {
        selectedSuggestionIndex = (selectedSuggestionIndex + 1) % currentSuggestions.length;
      } else {
        selectedSuggestionIndex = (selectedSuggestionIndex - 1 + currentSuggestions.length) % currentSuggestions.length;
      }
      updateSuggestionDisplay();
      return;
    }
    if (key === 'Enter') {
      e.preventDefault();
      // If suggestions exist, accept current selection first
      if (currentSuggestions.length > 0) {
        applySuggestion(false);
      }
      // Execute the command
      const cmdLine = inputSpan.textContent.trim();
      clearSuggestions();
      cursorSpan.remove();
      inputSpan = null;
      cursorSpan = null;
      if (cmdLine === "") {
        newPrompt();
      } else {
        runCommand(cmdLine);
      }
      return;
    }
    if (key === 'Backspace') {
      e.preventDefault();
      inputSpan.textContent = inputSpan.textContent.slice(0, -1);
      updateAutocomplete();
      return;
    }
    if (key === ' ') {
      e.preventDefault();
      if (currentSuggestions.length > 0) {
        // Accept suggestion and add space
        applySuggestion(true);
      } else {
        inputSpan.textContent += ' ';
        updateAutocomplete();
      }
      return;
    }
    // Character input
    if (key.length === 1) {
      e.preventDefault();
      inputSpan.textContent += key;
      updateAutocomplete();
      return;
    }
    // Other keys ignored
  }

  function boot() {
    // Add static introductory lines with normal spacing
    term.innerHTML = `jrj.io redirective shell v1.0<br>Type 'help' for a list of commands.`;
    newPrompt();
    // Add a 200ms delay before typing 'help' and pressing Enter
    setTimeout(() => {
      typeChars("help", () => handleKey({ key: "Enter", preventDefault: () => {} }));
    }, 500);
  }

  term.addEventListener("click", () => term.focus());
  document.addEventListener("keydown", handleKey);
  term.focus();

  document.addEventListener("click", (e) => {
    const el = e.target.closest(".clickable");
    if (el) {
      const command = el.dataset.command || el.textContent.trim();
      typeChars(command, () => handleKey({ key: "Enter", preventDefault: () => {} })); // Type the command and press Enter
    }
  });

  boot();
})();