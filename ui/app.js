(() => {
  const state = {
    todos: [],
    config: {
      trigger_key: "J",
      double_tap_ms: 250,
      launch_at_login: true,
      hide_completed: false,
      max_visible_plates: 40,
      min_diameter_px: 168,
      max_diameter_px: 440,
      complete_fade_ms: 700,
      input_hotkey: "N",
      list_mode_hotkey: "M",
    },
    viewMode: "explosion",
    overlayVisible: false,
    permissions: {
      input_monitoring: false,
      accessibility: false,
    },
    visibleCount: 0,
    hiddenCount: 0,
    completedCount: 0,
    inputOpen: false,
    searchQuery: "",
    permissionBannerDismissed: false,
    overflowEditorOpen: false,
    overflowEditorTodoId: null,
    modalImeComposing: false,
    overflowImeComposing: false,
    plateNodes: new Map(),
    completingIds: new Set(),
    completionFallbackTimers: new Map(),
    editingId: null,
    contextTargetId: null,
    emptyPlateNode: null,
    forceExplodeOnNextRender: false,
  };

  const el = {
    stage: document.getElementById("stage"),
    plateLayer: document.getElementById("plateLayer"),
    listModePanel: document.getElementById("listModePanel"),
    listModeList: document.getElementById("listModeList"),
    listSearch: document.getElementById("listSearch"),
    permissionBanner: document.getElementById("permissionBanner"),
    permissionCloseButton: document.getElementById("permissionCloseButton"),
    openPermissionButton: document.getElementById("openPermissionButton"),
    overflowEditor: document.getElementById("overflowEditor"),
    overflowEditorInput: document.getElementById("overflowEditorInput"),
    overflowEditorCancel: document.getElementById("overflowEditorCancel"),
    overflowEditorSave: document.getElementById("overflowEditorSave"),
    contextMenu: document.getElementById("contextMenu"),
    contextDeleteButton: document.getElementById("contextDeleteButton"),
    inputDock: document.getElementById("inputDock"),
    modalInput: document.getElementById("modalInput"),
    modalCancelButton: document.getElementById("modalCancelButton"),
    modalSubmitButton: document.getElementById("modalSubmitButton"),
    errorToast: document.getElementById("errorToast"),
  };

  if (typeof window.__TODOLITE_PLATE_IMAGE_DATA === "string" && window.__TODOLITE_PLATE_IMAGE_DATA.length > 0) {
    document.documentElement.style.setProperty(
      "--plate-image-url",
      `url('${window.__TODOLITE_PLATE_IMAGE_DATA}')`,
    );
  }

  let toastTimer = null;

  const post = (type, payload = {}) => {
    if (!window.ipc || typeof window.ipc.postMessage !== "function") {
      return;
    }
    window.ipc.postMessage(JSON.stringify({ type, payload }));
  };

  const sanitizeText = (text) => text.replace(/[\n\r\t]/g, " ").trim();

  const clamp = (value, min, max) => Math.max(min, Math.min(max, value));
  const isImeComposing = (event, composingFlag = false) =>
    !!(event?.isComposing || event?.keyCode === 229 || composingFlag);

  const plateTypography = (diameter) => ({
    fontSizePx: clamp(Math.round(diameter * 0.11), 12, 18),
    lineHeight: 1.28,
  });

  const stableHash = (text) => {
    let hash = 5381;
    for (let i = 0; i < text.length; i += 1) {
      hash = ((hash << 5) + hash + text.charCodeAt(i)) >>> 0;
    }
    return hash;
  };

  const isTypingTarget = () => {
    const active = document.activeElement;
    if (!active) return false;
    const tag = active.tagName;
    return tag === "INPUT" || tag === "TEXTAREA" || active.isContentEditable;
  };

  const getActiveTodos = () =>
    state.todos
      .filter((todo) => !todo.completed)
      .sort((a, b) => (b.created_at_ms || 0) - (a.created_at_ms || 0));

  const getVisibleTodos = () => {
    const maxVisible = Math.max(1, Number(state.config.max_visible_plates) || 40);
    const active = getActiveTodos();
    return active.slice(0, maxVisible);
  };

  const buildDiameterMap = (visibleTodos) => {
    const minDiameterRaw = clamp(Number(state.config.min_diameter_px) || 168, 168, 520);
    const maxDiameterRaw = clamp(Number(state.config.max_diameter_px) || 440, 320, 760);
    const densityScale = 0.78;
    const minDiameter = clamp(Math.round(minDiameterRaw * densityScale), 131, 420);
    const maxDiameter = clamp(Math.round(maxDiameterRaw * densityScale), 250, 620);
    const low = Math.min(minDiameter, maxDiameter);
    const high = Math.max(minDiameter, maxDiameter);

    const byAge = [...visibleTodos].sort(
      (a, b) => (a.created_at_ms || 0) - (b.created_at_ms || 0),
    );

    const result = new Map();
    if (byAge.length === 1) {
      result.set(byAge[0].id, high);
      return result;
    }

    byAge.forEach((todo, index) => {
      const ratio = index / (byAge.length - 1);
      const diameter = high - ratio * (high - low);
      result.set(todo.id, clamp(Math.round(diameter), low, high));
    });

    return result;
  };

  const buildRadialLayout = (visibleTodos, diameterById) => {
    const rect = el.stage.getBoundingClientRect();
    const width = rect.width || window.innerWidth;
    const height = rect.height || window.innerHeight;

    const centerX = width / 2;
    const centerY = height / 2;

    const margin = 14;
    const topReserved = 14;
    const gap = 5;
    const bottomReserved = state.inputOpen ? 84 : 16;
    const golden = Math.PI * (3 - Math.sqrt(5));
    const radialStep = 18;

    const placementOrder = [...visibleTodos].sort(
      (a, b) => (diameterById.get(b.id) || 0) - (diameterById.get(a.id) || 0),
    );

    const layoutById = new Map();
    const placed = [];

    placementOrder.forEach((todo) => {
      const diameter = diameterById.get(todo.id) || 160;
      const radius = diameter / 2;
      const jitter = (stableHash(todo.id) % 1000) / 1000;

      let chosen = null;
      for (let i = 0; i < 3200; i += 1) {
        const orbit = radialStep * Math.sqrt(i + 1);
        const theta = i * golden + jitter * Math.PI * 2;
        const x = centerX + Math.cos(theta) * orbit;
        const y = centerY + Math.sin(theta) * orbit;

        if (x - radius < margin || x + radius > width - margin) continue;
        if (y - radius < topReserved || y + radius > height - margin - bottomReserved) continue;

        const overlap = placed.some((other) => {
          const dx = other.x - x;
          const dy = other.y - y;
          const distance = Math.hypot(dx, dy);
          return distance < other.radius + radius + gap;
        });

        if (!overlap) {
          chosen = { x, y };
          break;
        }
      }

      if (!chosen) {
        const fallbackX = clamp(centerX + ((jitter - 0.5) * width) / 3, radius + margin, width - margin - radius);
        const fallbackY = clamp(
          centerY + ((0.5 - jitter) * height) / 3,
          radius + topReserved,
          height - margin - bottomReserved - radius,
        );
        chosen = { x: fallbackX, y: fallbackY };
      }

      placed.push({ x: chosen.x, y: chosen.y, radius });
      layoutById.set(todo.id, {
        x: chosen.x,
        y: chosen.y,
        diameter,
      });
    });

    return {
      centerX,
      centerY,
      layoutById,
    };
  };

  const formatRelative = (ms) => {
    if (!ms) return "";
    const delta = Date.now() - ms;
    const min = Math.floor(delta / 60000);
    if (min < 1) return "刚刚";
    if (min < 60) return `${min} 分钟前`;
    const hour = Math.floor(min / 60);
    if (hour < 24) return `${hour} 小时前`;
    const day = Math.floor(hour / 24);
    return `${day} 天前`;
  };

  const showError = (message) => {
    if (!message) return;
    el.errorToast.textContent = message;
    el.errorToast.classList.add("show");
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      el.errorToast.classList.remove("show");
    }, 2400);
  };

  const closeContextMenu = () => {
    state.contextTargetId = null;
    el.contextMenu.classList.add("hidden");
  };

  const openContextMenu = (id, x, y) => {
    state.contextTargetId = id;
    el.contextMenu.classList.remove("hidden");
    el.contextMenu.style.left = `${x + 6}px`;
    el.contextMenu.style.top = `${y + 6}px`;
  };

  const closeOverflowEditor = () => {
    state.overflowEditorOpen = false;
    state.overflowEditorTodoId = null;
    state.overflowImeComposing = false;
    el.overflowEditor.classList.add("hidden");
    el.overflowEditor.setAttribute("aria-hidden", "true");
    el.overflowEditor.style.left = "";
    el.overflowEditor.style.top = "";
  };

  const positionOverflowEditor = (plate) => {
    if (!plate) return;
    const gap = 10;
    const margin = 12;
    const stageRect = el.stage.getBoundingClientRect();
    const plateRect = plate.getBoundingClientRect();
    const editorRect = el.overflowEditor.getBoundingClientRect();

    let x = plateRect.right + gap;
    if (x + editorRect.width > stageRect.width - margin) {
      x = plateRect.left - editorRect.width - gap;
    }
    x = clamp(x, margin, stageRect.width - editorRect.width - margin);

    let y = plateRect.top + (plateRect.height - editorRect.height) / 2;
    y = clamp(y, margin, stageRect.height - editorRect.height - margin);

    el.overflowEditor.style.left = `${Math.round(x)}px`;
    el.overflowEditor.style.top = `${Math.round(y)}px`;
  };

  const openOverflowEditor = (todoId, plate) => {
    const todo = state.todos.find((item) => item.id === todoId && !item.completed);
    if (!todo || !plate) return;

    if (state.editingId) {
      state.editingId = null;
    }
    if (state.inputOpen) {
      closeInputModal();
    }
    closeContextMenu();

    state.overflowEditorOpen = true;
    state.overflowEditorTodoId = todoId;

    el.overflowEditorInput.value = todo.text;
    el.overflowEditor.classList.remove("hidden");
    el.overflowEditor.setAttribute("aria-hidden", "false");

    window.requestAnimationFrame(() => {
      if (!state.overflowEditorOpen || state.overflowEditorTodoId !== todoId) return;
      positionOverflowEditor(plate);
      el.overflowEditorInput.focus();
      el.overflowEditorInput.select();
    });
  };

  const submitOverflowEditor = () => {
    if (!state.overflowEditorOpen || !state.overflowEditorTodoId) return;

    const text = sanitizeText(el.overflowEditorInput.value || "");
    if (!text) {
      closeOverflowEditor();
      return;
    }

    post("edit_todo", { id: state.overflowEditorTodoId, text });
    closeOverflowEditor();
  };

  const isPlateTextOverflowing = (plate, text) => {
    const label = plate.querySelector(".plate-label");
    if (!label) return false;
    const heightOverflow = label.scrollHeight > label.clientHeight + 1;
    const widthOverflow = label.scrollWidth > label.clientWidth + 1;
    const fontSize = Number.parseFloat(window.getComputedStyle(label).fontSize) || 14;
    const charsPerLine = Math.max(6, Math.floor(label.clientWidth / (fontSize * 0.58)));
    const approxCapacity = charsPerLine * 2;
    const lengthOverflow = (text || "").length > approxCapacity;
    return heightOverflow || widthOverflow || lengthOverflow;
  };

  const closeInputModal = () => {
    state.inputOpen = false;
    state.modalImeComposing = false;
    el.inputDock.classList.add("hidden");
    el.inputDock.setAttribute("aria-hidden", "true");
    el.modalInput.value = "";
    if (state.overflowEditorOpen) {
      closeOverflowEditor();
    }
    if (state.overlayVisible && state.viewMode === "explosion") {
      renderExplosion(false);
    }
  };

  const openInputModal = () => {
    if (state.overflowEditorOpen) {
      closeOverflowEditor();
    }
    state.inputOpen = true;
    el.inputDock.classList.remove("hidden");
    el.inputDock.setAttribute("aria-hidden", "false");
    if (state.overlayVisible && state.viewMode === "explosion") {
      renderExplosion(false);
    }
    window.setTimeout(() => {
      el.modalInput.focus();
      el.modalInput.select();
    }, 20);
  };

  const submitInputModal = () => {
    const text = sanitizeText(el.modalInput.value);
    if (!text) {
      el.modalInput.focus();
      return;
    }
    post("add_todo", { text });
    closeInputModal();
  };

  const clearCompletionFallback = (id) => {
    const timer = state.completionFallbackTimers.get(id);
    if (timer) {
      clearTimeout(timer);
      state.completionFallbackTimers.delete(id);
    }
  };

  const triggerComplete = (id) => {
    if (state.completingIds.has(id)) return;
    if (state.overflowEditorTodoId === id) {
      closeOverflowEditor();
    }

    state.completingIds.add(id);
    renderExplosion(false);

    const fadeMs = clamp(Number(state.config.complete_fade_ms) || 700, 200, 3000);

    window.setTimeout(() => {
      post("complete_todo", { id, via: "cmd_click" });
    }, fadeMs);

    clearCompletionFallback(id);
    const fallback = window.setTimeout(() => {
      state.completingIds.delete(id);
      state.completionFallbackTimers.delete(id);
      renderExplosion(false);
    }, fadeMs + 1800);
    state.completionFallbackTimers.set(id, fallback);
  };

  const commitEdit = (id, value) => {
    const text = sanitizeText(value || "");
    if (!text) {
      state.editingId = null;
      renderExplosion(false);
      return;
    }

    post("edit_todo", { id, text });
    state.editingId = null;
    renderExplosion(false);
  };

  const mountPlateContent = (plate, todo) => {
    plate.innerHTML = "";

    const isEditing = state.editingId === todo.id;
    if (isEditing) {
      const input = document.createElement("input");
      input.className = "plate-editor";
      input.type = "text";
      input.maxLength = 120;
      input.value = todo.text;
      let inlineImeComposing = false;
      input.addEventListener("compositionstart", () => {
        inlineImeComposing = true;
      });
      input.addEventListener("compositionend", () => {
        inlineImeComposing = false;
      });
      input.addEventListener("keydown", (event) => {
        if (isImeComposing(event, inlineImeComposing)) {
          return;
        }
        if (event.key === "Enter") {
          event.preventDefault();
          commitEdit(todo.id, input.value);
        } else if (event.key === "Escape") {
          event.preventDefault();
          state.editingId = null;
          renderExplosion(false);
        }
      });
      input.addEventListener("blur", () => {
        commitEdit(todo.id, input.value);
      });
      plate.appendChild(input);

      window.setTimeout(() => {
        input.focus();
        input.select();
      }, 0);
      return;
    }

    const label = document.createElement("div");
    label.className = "plate-label";
    label.textContent = todo.text;
    label.title = todo.text;

    const meta = document.createElement("div");
    meta.className = "plate-meta";
    meta.textContent = formatRelative(todo.created_at_ms);

    plate.appendChild(label);
    plate.appendChild(meta);
  };

  const createPlateNode = () => {
    const plate = document.createElement("article");
    plate.className = "plate";

    plate.addEventListener("click", (event) => {
      const id = plate.dataset.id;
      if (!id) return;

      closeContextMenu();

      if (event.metaKey) {
        event.preventDefault();
        event.stopPropagation();
        triggerComplete(id);
      }
    });

    plate.addEventListener("dblclick", (event) => {
      if (event.metaKey) return;
      const id = plate.dataset.id;
      if (!id || state.completingIds.has(id)) return;
      const shouldUseOverflowEditor = plate.dataset.overflow === "1";
      if (shouldUseOverflowEditor) {
        openOverflowEditor(id, plate);
      } else {
        closeOverflowEditor();
        state.editingId = id;
        renderExplosion(false);
      }
      event.preventDefault();
      event.stopPropagation();
    });

    plate.addEventListener("contextmenu", (event) => {
      const id = plate.dataset.id;
      if (!id) return;
      event.preventDefault();
      openContextMenu(id, event.clientX, event.clientY);
    });

    return plate;
  };

  const ensureEmptyPlate = (centerX, centerY) => {
    let node = state.emptyPlateNode;
    if (!node) {
      node = document.createElement("article");
      node.className = "plate plate--empty";
      node.setAttribute("role", "button");
      node.setAttribute("aria-label", "新增代办");
      node.innerHTML = `
        <div class="plate-label">按 N 新建代办</div>
        <div class="plate-meta">双击 J 触发舞台 · Esc 收起</div>
      `;
      node.addEventListener("click", (event) => {
        event.preventDefault();
        event.stopPropagation();
        openInputModal();
      });
      el.plateLayer.appendChild(node);
      state.emptyPlateNode = node;
    }

    const diameter = clamp((Number(state.config.min_diameter_px) || 168) + 72, 220, 340);
    const typo = plateTypography(diameter);
    node.style.width = `${diameter}px`;
    node.style.height = `${diameter}px`;
    node.style.left = `${centerX - diameter / 2}px`;
    node.style.top = `${centerY - diameter / 2}px`;
    node.style.setProperty("--tx", "0px");
    node.style.setProperty("--ty", "0px");
    node.style.setProperty("--travel-ms", "360ms");
    node.style.setProperty("--travel-delay", "0ms");
    node.style.setProperty("--plate-font-size", `${typo.fontSizePx}px`);
    node.style.setProperty("--plate-line-height", `${typo.lineHeight}`);
    node.classList.add("plate--placed");
  };

  const removeEmptyPlate = () => {
    if (state.emptyPlateNode) {
      state.emptyPlateNode.remove();
      state.emptyPlateNode = null;
    }
  };

  const renderExplosion = (forceExplode) => {
    const shouldForce = !!forceExplode || state.forceExplodeOnNextRender;
    state.forceExplodeOnNextRender = false;

    const visibleTodos = getVisibleTodos();
    const diameterById = buildDiameterMap(visibleTodos);
    const { centerX, centerY, layoutById } = buildRadialLayout(visibleTodos, diameterById);

    const visibleIds = new Set(visibleTodos.map((todo) => todo.id));

    for (const [id, node] of state.plateNodes) {
      if (!visibleIds.has(id)) {
        node.remove();
        state.plateNodes.delete(id);
        state.completingIds.delete(id);
        clearCompletionFallback(id);
        if (state.overflowEditorTodoId === id) {
          closeOverflowEditor();
        }
      }
    }

    if (visibleTodos.length === 0) {
      ensureEmptyPlate(centerX, centerY);
      return;
    }
    removeEmptyPlate();

    visibleTodos.forEach((todo) => {
      const layout = layoutById.get(todo.id);
      if (!layout) return;

      let plate = state.plateNodes.get(todo.id);
      const isNew = !plate;
      if (!plate) {
        plate = createPlateNode();
        state.plateNodes.set(todo.id, plate);
        el.plateLayer.appendChild(plate);
      }

      plate.dataset.id = todo.id;
      plate.style.width = `${layout.diameter}px`;
      plate.style.height = `${layout.diameter}px`;
      plate.style.left = `${centerX - layout.diameter / 2}px`;
      plate.style.top = `${centerY - layout.diameter / 2}px`;
      plate.style.setProperty("--center-x", `${centerX - layout.diameter / 2}px`);
      plate.style.setProperty("--center-y", `${centerY - layout.diameter / 2}px`);
      plate.style.setProperty("--tx", `${layout.x - centerX}px`);
      plate.style.setProperty("--ty", `${layout.y - centerY}px`);
      const typo = plateTypography(layout.diameter);
      plate.style.setProperty("--plate-font-size", `${typo.fontSizePx}px`);
      plate.style.setProperty("--plate-line-height", `${typo.lineHeight}`);

      const hash = stableHash(todo.id);
      const duration = 420 + (hash % 360);
      const delay = isNew ? hash % 80 : 0;
      plate.style.setProperty("--travel-ms", `${duration}ms`);
      plate.style.setProperty("--travel-delay", `${delay}ms`);

      plate.classList.toggle("plate--completing", state.completingIds.has(todo.id));
      plate.classList.toggle("plate--editing", state.editingId === todo.id);

      mountPlateContent(plate, todo);
      plate.dataset.overflow = isPlateTextOverflowing(plate, todo.text) ? "1" : "0";

      if (isNew || shouldForce) {
        plate.classList.remove("plate--placed");
        window.requestAnimationFrame(() => {
          plate.classList.add("plate--placed");
        });
      } else {
        plate.classList.add("plate--placed");
      }

      if (state.overflowEditorOpen && state.overflowEditorTodoId === todo.id) {
        positionOverflowEditor(plate);
      }
    });
  };

  const renderListMode = () => {
    removeEmptyPlate();
    const active = getActiveTodos();
    const query = state.searchQuery.trim().toLocaleLowerCase();
    const rows = query
      ? active.filter((todo) => todo.text.toLocaleLowerCase().includes(query))
      : active;
    el.listModeList.innerHTML = "";

    if (rows.length === 0) {
      const empty = document.createElement("li");
      empty.className = "list-empty";
      empty.textContent = query ? "没有匹配的未完成任务" : "暂无未完成任务";
      el.listModeList.appendChild(empty);
      return;
    }

    rows.forEach((todo) => {
      const row = document.createElement("li");
      row.className = "list-mode-row";

      const text = document.createElement("div");
      text.className = "list-text";
      text.textContent = todo.text;
      text.title = todo.text;
      text.addEventListener("dblclick", () => {
        state.editingId = todo.id;
        state.viewMode = "explosion";
        post("set_view_mode", { mode: "explosion" });
      });

      const doneButton = document.createElement("button");
      doneButton.className = "list-row-btn";
      doneButton.type = "button";
      doneButton.textContent = "完成";
      doneButton.addEventListener("click", () => {
        triggerComplete(todo.id);
      });

      const deleteButton = document.createElement("button");
      deleteButton.className = "list-row-btn danger";
      deleteButton.type = "button";
      deleteButton.textContent = "删除";
      deleteButton.addEventListener("click", () => {
        post("delete_todo", { id: todo.id });
      });

      row.appendChild(text);
      row.appendChild(doneButton);
      row.appendChild(deleteButton);
      el.listModeList.appendChild(row);
    });
  };

  const renderPermissions = () => {
    const allGranted =
      state.permissions.input_monitoring && state.permissions.accessibility;
    if (allGranted) {
      el.permissionBanner.classList.add("hidden");
      return;
    }
    el.permissionBanner.classList.toggle("hidden", state.permissionBannerDismissed);
  };

  const renderStatus = () => {
    // Explosion mode intentionally keeps the stage clean with no persistent HUD.
  };

  const renderViewMode = () => {
    const isList = state.viewMode === "list";
    el.listModePanel.classList.toggle("hidden", !isList);
    if (isList && state.overflowEditorOpen) {
      closeOverflowEditor();
    }
    if (el.listSearch.value !== state.searchQuery) {
      el.listSearch.value = state.searchQuery;
    }

    if (isList) {
      renderListMode();
    }
  };

  const renderAll = () => {
    renderStatus();
    renderPermissions();
    renderViewMode();
    renderExplosion(false);
  };

  window.__TODOLITE_HANDLE_RUST = (message) => {
    if (!message || typeof message !== "object") {
      return;
    }

    if (message.type === "state_sync") {
      const payload = message.payload || {};
      const previousOverlay = state.overlayVisible;

      state.todos = Array.isArray(payload.todos) ? payload.todos : [];
      state.config = {
        ...state.config,
        ...(payload.config || {}),
      };
      state.viewMode = payload.view_mode || state.viewMode || "explosion";
      state.overlayVisible = !!payload.overlay_visible;
      state.visibleCount = Number(payload.visible_count ?? 0);
      state.hiddenCount = Number(payload.hidden_count ?? 0);
      state.completedCount = Number(payload.completed_count ?? 0);

      if (!previousOverlay && state.overlayVisible) {
        state.forceExplodeOnNextRender = true;
      }

      const activeIds = new Set(state.todos.filter((todo) => !todo.completed).map((todo) => todo.id));
      for (const id of state.completingIds) {
        if (!activeIds.has(id)) {
          state.completingIds.delete(id);
          clearCompletionFallback(id);
        }
      }

      renderAll();
      return;
    }

    if (message.type === "permission_state") {
      state.permissions = {
        ...state.permissions,
        ...(message.payload || {}),
      };
      renderPermissions();
      return;
    }

    if (message.type === "error") {
      showError(message.payload?.message || "操作失败");
      return;
    }

    if (message.type === "open_input_modal") {
      openInputModal();
    }
  };

  window.__TODOLITE_SET_OVERLAY_VISIBLE = (visible) => {
    document.documentElement.classList.toggle("overlay-visible", !!visible);

    const wasVisible = state.overlayVisible;
    state.overlayVisible = !!visible;

    if (state.overlayVisible && !wasVisible) {
      state.forceExplodeOnNextRender = true;
      renderExplosion(true);
    }

    if (!state.overlayVisible) {
      closeInputModal();
      closeContextMenu();
      closeOverflowEditor();
      state.editingId = null;
      state.searchQuery = "";
      el.listSearch.value = "";
    }
  };

  if (Array.isArray(window.__TODOLITE_QUEUE)) {
    while (window.__TODOLITE_QUEUE.length > 0) {
      const queued = window.__TODOLITE_QUEUE.shift();
      window.__TODOLITE_HANDLE_RUST(queued);
    }
  }

  el.permissionCloseButton.addEventListener("click", (event) => {
    event.stopPropagation();
    state.permissionBannerDismissed = true;
    renderPermissions();
  });

  el.openPermissionButton.addEventListener("click", () => {
    post("open_permissions");
  });

  el.contextDeleteButton.addEventListener("click", () => {
    if (state.contextTargetId) {
      if (state.overflowEditorTodoId === state.contextTargetId) {
        closeOverflowEditor();
      }
      post("delete_todo", { id: state.contextTargetId });
    }
    closeContextMenu();
  });

  el.inputDock.addEventListener("click", (event) => {
    event.stopPropagation();
  });

  el.modalCancelButton.addEventListener("click", () => {
    closeInputModal();
  });

  el.modalSubmitButton.addEventListener("click", () => {
    submitInputModal();
  });

  el.modalInput.addEventListener("keydown", (event) => {
    if (isImeComposing(event, state.modalImeComposing)) {
      return;
    }
    if (event.key === "Enter") {
      event.preventDefault();
      submitInputModal();
    } else if (event.key === "Escape") {
      event.preventDefault();
      closeInputModal();
    }
  });

  el.listSearch.addEventListener("input", () => {
    state.searchQuery = el.listSearch.value || "";
    if (state.viewMode === "list") {
      renderListMode();
    }
  });

  el.overflowEditorCancel.addEventListener("click", () => {
    closeOverflowEditor();
  });

  el.overflowEditorSave.addEventListener("click", () => {
    submitOverflowEditor();
  });

  el.overflowEditorInput.addEventListener("keydown", (event) => {
    if (isImeComposing(event, state.overflowImeComposing)) {
      return;
    }
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      submitOverflowEditor();
      return;
    }
    if (event.key === "Escape") {
      event.preventDefault();
      closeOverflowEditor();
    }
  });

  el.modalInput.addEventListener("compositionstart", () => {
    state.modalImeComposing = true;
  });
  el.modalInput.addEventListener("compositionend", () => {
    state.modalImeComposing = false;
  });

  el.overflowEditorInput.addEventListener("compositionstart", () => {
    state.overflowImeComposing = true;
  });
  el.overflowEditorInput.addEventListener("compositionend", () => {
    state.overflowImeComposing = false;
  });

  document.addEventListener("click", (event) => {
    if (!el.contextMenu.contains(event.target)) {
      closeContextMenu();
    }
  });

  el.stage.addEventListener("click", (event) => {
    const target = event.target;
    if (!(target instanceof Element)) return;

    if (target.closest(".plate, .plate-editor, .input-dock, .overflow-editor, .list-mode-panel, .context-menu, .permission-banner")) {
      return;
    }

    if (state.overflowEditorOpen) {
      closeOverflowEditor();
      return;
    }

    post("hide_overlay");
  });

  window.addEventListener("resize", () => {
    if (state.overlayVisible) {
      renderExplosion(false);
    }
    if (state.overflowEditorOpen && state.overflowEditorTodoId) {
      const anchorPlate = state.plateNodes.get(state.overflowEditorTodoId);
      if (anchorPlate) {
        positionOverflowEditor(anchorPlate);
      } else {
        closeOverflowEditor();
      }
    }
    closeContextMenu();
  });

  window.addEventListener("keydown", (event) => {
    if (!state.overlayVisible) return;

    if (event.key === "Escape") {
      if (!el.contextMenu.classList.contains("hidden")) {
        closeContextMenu();
        event.preventDefault();
        return;
      }
      if (state.inputOpen) {
        closeInputModal();
        event.preventDefault();
        return;
      }
      if (state.overflowEditorOpen) {
        closeOverflowEditor();
        event.preventDefault();
        return;
      }
      if (state.editingId) {
        state.editingId = null;
        renderExplosion(false);
        event.preventDefault();
        return;
      }
      post("hide_overlay");
      event.preventDefault();
      return;
    }

    if (isTypingTarget()) return;

    const key = event.key.toUpperCase();
    const inputHotkey = (state.config.input_hotkey || "N").toUpperCase();
    const listHotkey = (state.config.list_mode_hotkey || "M").toUpperCase();

    if (key === inputHotkey) {
      openInputModal();
      event.preventDefault();
      return;
    }

    if (key === listHotkey) {
      if (state.overflowEditorOpen) {
        closeOverflowEditor();
      }
      if (state.inputOpen) {
        closeInputModal();
      }
      const next = state.viewMode === "list" ? "explosion" : "list";
      post("set_view_mode", { mode: next });
      event.preventDefault();
    }
  });

  renderAll();
  post("request_state");
})();
