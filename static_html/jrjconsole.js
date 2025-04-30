(() => {
  const term = document.getElementById("terminal");
  const charDelay = 3;
  // Generate a random IPv4 address for fun hacker alerts
  function randomIP() {
    return Array(4).fill(0)
      .map(() => Math.floor(Math.random() * 256))
      .join('.');
  }

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
      if(args[0]) {
        window.open(args[0], "_blank");
        print("Opening " + args[0]);
      } else {
        printLines(["Usage: open <url>"]); 
      }
      newPrompt();
    },

    ls: (args) => printLines([
      "ferris.png  index.html  jrjconsole.js  styles.css  wally.png",
      " "
    ]),
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

  function newPrompt() {
    const wrapper = document.createElement("div");
    wrapper.innerHTML = `<span class="prompt">$ </span><span id="input"></span>`;
    inputSpan = wrapper.querySelector("#input");
    cursorSpan = document.createElement("span");
    cursorSpan.className = "cursor";
    wrapper.appendChild(cursorSpan); // Append cursor after input
    term.appendChild(wrapper);
    scrollBottom();
  }

  function handleKey(e) {
    if (!inputSpan) return;
    e.preventDefault();
    if (e.key === "Backspace") {
      inputSpan.textContent = inputSpan.textContent.slice(0, -1);
    } else if (e.key === "Enter") {
      const cmdLine = inputSpan.textContent.trim();
      if (cmdLine === "") {
        newPrompt(); // Create a new prompt if no command is entered
        return;
      }
      cursorSpan.remove(); // Remove cursor after command
      inputSpan = null;
      cursorSpan = null;
      runCommand(cmdLine);
    } else if (e.key.length === 1) {
      inputSpan.textContent += e.key;
    }
    scrollBottom();
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