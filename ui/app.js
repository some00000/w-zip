const state = {
  mode: "seal",
  files: [],
  archivePath: "",
  folderPath: "",
  busy: false,
};

const statusEl = document.querySelector("#status");
const fileList = document.querySelector("#file-list");
const fileCount = document.querySelector("#file-count");

function invoke(command, args) {
  const core = window.__TAURI__ && window.__TAURI__.core;
  if (!core || typeof core.invoke !== "function") {
    throw new Error("Open this screen in the w+zip app.");
  }
  return core.invoke(command, args);
}

function messageOf(error) {
  if (typeof error === "string") return error;
  if (error && typeof error.message === "string") return error.message;
  return "Something went wrong.";
}

function setStatus(message, kind) {
  statusEl.textContent = message;
  statusEl.className = kind || "";
}

function baseName(path) {
  const parts = String(path).split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

function setBusy(busy) {
  state.busy = busy;
  document.querySelectorAll("button").forEach((button) => {
    button.disabled = busy;
  });
}

function renderFiles() {
  fileList.replaceChildren();
  state.files.forEach((path, index) => {
    const item = document.createElement("li");
    const name = document.createElement("div");
    name.className = "name";
    const strong = document.createElement("strong");
    strong.textContent = baseName(path);
    const full = document.createElement("span");
    full.textContent = path;
    name.append(strong, full);

    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "remove";
    remove.textContent = "Remove";
    remove.addEventListener("click", () => {
      state.files.splice(index, 1);
      renderFiles();
    });
    item.append(name, remove);
    fileList.append(item);
  });
  const count = state.files.length;
  fileCount.textContent =
    count === 0 ? "No files yet." : count === 1 ? "1 file." : `${count} files.`;
}

function showMode(mode) {
  state.mode = mode;
  const seal = mode === "seal";
  document.querySelector("#panel-seal").hidden = !seal;
  document.querySelector("#panel-open").hidden = seal;
  document.querySelector("#tab-seal").setAttribute("aria-selected", String(seal));
  document.querySelector("#tab-open").setAttribute("aria-selected", String(!seal));
  setStatus("");
}

document.querySelector("#tab-seal").addEventListener("click", () => showMode("seal"));
document.querySelector("#tab-open").addEventListener("click", () => showMode("open"));

document.querySelector("#add-files").addEventListener("click", async () => {
  if (state.busy) return;
  setBusy(true);
  try {
    const picked = await invoke("pick_files");
    let added = 0;
    for (const path of picked) {
      if (!state.files.includes(path)) {
        state.files.push(path);
        added += 1;
      }
    }
    renderFiles();
    if (added > 0) setStatus("");
  } catch (error) {
    setStatus(messageOf(error), "error");
  } finally {
    setBusy(false);
  }
});

document.querySelector("#seal").addEventListener("click", async () => {
  if (state.busy) return;
  const password = document.querySelector("#seal-password").value;
  const confirm = document.querySelector("#seal-confirm").value;
  if (!state.files.length) {
    setStatus("Choose at least one file.", "error");
    return;
  }
  if (!password) {
    setStatus("Enter a password.", "error");
    return;
  }
  if (password !== confirm) {
    setStatus("The passphrases do not match.", "error");
    return;
  }

  setBusy(true);
  try {
    const destination = await invoke("pick_save_path");
    if (!destination) {
      setStatus("Export cancelled.", "");
      return;
    }
    const report = await invoke("pack_wzip", {
      sources: state.files,
      password,
      destination,
    });
    document.querySelector("#seal-password").value = "";
    document.querySelector("#seal-confirm").value = "";
    setStatus(
      `Sealed ${report.fileCount} ${report.fileCount === 1 ? "file" : "files"} to ${report.destination}.`,
      "ok",
    );
  } catch (error) {
    setStatus(messageOf(error), "error");
  } finally {
    setBusy(false);
  }
});

document.querySelector("#choose-archive").addEventListener("click", async () => {
  if (state.busy) return;
  setBusy(true);
  try {
    const picked = await invoke("pick_wzip");
    if (picked) {
      state.archivePath = picked;
      document.querySelector("#archive-path").textContent = picked;
      setStatus("");
    }
  } catch (error) {
    setStatus(messageOf(error), "error");
  } finally {
    setBusy(false);
  }
});

document.querySelector("#choose-folder").addEventListener("click", async () => {
  if (state.busy) return;
  setBusy(true);
  try {
    const picked = await invoke("pick_folder");
    if (picked) {
      state.folderPath = picked;
      document.querySelector("#folder-path").textContent = picked;
      setStatus("");
    }
  } catch (error) {
    setStatus(messageOf(error), "error");
  } finally {
    setBusy(false);
  }
});

document.querySelector("#extract").addEventListener("click", async () => {
  if (state.busy) return;
  const password = document.querySelector("#open-password").value;
  if (!state.archivePath) {
    setStatus("Choose a .wzip archive.", "error");
    return;
  }
  if (!password) {
    setStatus("Enter a password.", "error");
    return;
  }
  if (!state.folderPath) {
    setStatus("Choose a folder for the extracted files.", "error");
    return;
  }

  setBusy(true);
  try {
    const report = await invoke("unpack_wzip", {
      archive: state.archivePath,
      password,
      destination: state.folderPath,
    });
    document.querySelector("#open-password").value = "";
    const count = report.files.length;
    setStatus(
      `Extracted ${count} ${count === 1 ? "file" : "files"} to ${report.destination}.`,
      "ok",
    );
  } catch (error) {
    setStatus(messageOf(error), "error");
  } finally {
    setBusy(false);
  }
});

renderFiles();
