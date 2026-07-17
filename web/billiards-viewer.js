(() => {
  const tableDetailGroups = {
    cloth: '.table-cloth, .table-cloth-texture',
    clothTexture: '.table-cloth-texture',
    rail: '.table-rail, .table-rail-grain, .table-rail-grain-horizontal, .table-rail-grain-vertical, .table-rail-inner-shadow',
    railTexture: '.table-rail-grain, .table-rail-grain-horizontal, .table-rail-grain-vertical, .table-rail-inner-shadow',
    cushions: '.table-cushion, .table-cushion-nose, .table-cushion-back',
    pockets: '.table-pocket-well, .table-pocket-leather, .table-pocket-leather-highlight, .table-pocket-shelf, .table-pocket-shelf-texture, .table-pocket-facing',
    pocketTexture: '.table-pocket-shelf-texture',
    diamonds: '.table-diamond',
  };
  const tableDetailAllSelector = Array.from(new Set(Object.values(tableDetailGroups).join(',').split(',').map((selector) => selector.trim()))).join(',');
  const tableDetailOptions = [
    ['full', 'Full material'],
    ['flat', 'Flat colors'],
    ['cloth', 'Cloth only'],
    ['rail', 'Rails and pockets only'],
  ];
  const playbackHelpText = 'Scrub the physics frames in either direction, set playback speed from 1x down to 1/16x for slow motion, toggle Trace paths to hide static trajectory lines, or use the icon buttons: rewind to the first frame, step one frame back or forward, play/pause, or play to the next logged event. The default 2.5 ms physics frames update at about 25 frame changes per second at 1/16x. Balls are sampled by the Rust physics solver; black ticks show instantaneous travel direction. Spin badges use green arrows for natural roll, blue for follow, orange for draw, amber for skid, purple arcs for side spin, and a gray X for no spin.';

  const escapeHtml = (value) => String(value ?? '')
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');

  const escapeJsonScript = (value) => JSON.stringify(value)
    .replaceAll('<', '\\u003c')
    .replaceAll('>', '\\u003e')
    .replaceAll('&', '\\u0026');

  const tableDetailSelectHtml = (tableDetailDefault = 'full') => {
    const includeGlobal = tableDetailDefault === 'global';
    const options = includeGlobal ? [['global', 'Page setting'], ...tableDetailOptions] : tableDetailOptions;
    return `<label>Table detail<select data-table-detail>${options.map(([value, label]) => `<option value="${value}"${value === tableDetailDefault ? ' selected' : ''}>${label}</option>`).join('')}</select></label>`;
  };

  function viewerControlsHtml({ tableDetailDefault = 'full' } = {}) {
    return `<div class="viewer-controls" data-viewer-controls aria-label="Diagram controls">
        <button type="button" data-zoom="in">Zoom in</button>
        <button type="button" data-zoom="out">Zoom out</button>
        <button type="button" data-zoom="reset">Reset</button>
        <label><input type="checkbox" data-layer-toggle="table" checked>Table</label>
        <label><input type="checkbox" data-layer-toggle="overlays-below-balls" checked>Below-ball overlays</label>
        <label><input type="checkbox" data-layer-toggle="balls" checked>Balls</label>
        <label><input type="checkbox" data-layer-toggle="overlays-above-balls" checked>Above-ball overlays</label>
        ${tableDetailSelectHtml(tableDetailDefault)}
      </div>`;
  }

  const playbackControlsHtml = (maxFrame) => `<div class="playback-controls" aria-label="Playback controls">
        <button type="button" data-playback-reset aria-label="Rewind to beginning" title="Rewind to beginning">⏮</button>
        <button type="button" data-playback-step="-1" aria-label="Step back one frame" title="Step back">⏪</button>
        <button type="button" data-playback-play aria-label="Play" title="Play">▶</button>
        <button type="button" data-playback-step="1" aria-label="Step forward one frame" title="Step forward">⏩</button>
        <button type="button" data-playback-next-event aria-label="Play to next event" title="Next event">⏭</button>
        <label class="playback-speed-control">Speed <input type="range" data-playback-speed min="0.0625" max="1" value="1" step="0.0625" aria-label="Playback speed"><span class="playback-speed-value" data-playback-speed-label>1x</span></label>
        <label class="playback-trace-control"><input type="checkbox" data-playback-trace checked>Trace paths</label>
        <input type="range" data-playback-slider min="0" max="${maxFrame}" value="${maxFrame}" step="1" aria-label="Trace frame">
        <span class="playback-time" data-playback-time>t=0.000s</span>
        <span class="playback-event" data-playback-event>No events</span>
      </div>
      <p class="playback-help">${escapeHtml(playbackHelpText)}</p>`;

  function playbackPanelHtml(playback) {
    if (!playback || !Array.isArray(playback.frames) || playback.frames.length === 0) return '';
    const maxFrame = Math.max(0, playback.frames.length - 1);
    return `<div class="playback-panel" data-playback>
      <script type="application/json" data-playback-data>${escapeJsonScript(playback)}</script>
      ${playbackControlsHtml(maxFrame)}
    </div>`;
  }

  const hydrateViewerControls = (viewer) => {
    const placeholder = viewer.querySelector('[data-viewer-controls]');
    if (!placeholder || placeholder.querySelector('[data-zoom]')) return;
    const tableDetailDefault = placeholder.dataset.tableDetailDefault ?? 'full';
    placeholder.outerHTML = viewerControlsHtml({ tableDetailDefault });
  };

  const hydratePlaybackControls = (playbackPanel, playback) => {
    if (!playbackPanel || playbackPanel.querySelector('[data-playback-slider]')) return;
    playbackPanel.insertAdjacentHTML('beforeend', playbackControlsHtml(Math.max(0, playback.frames.length - 1)));
  };

  function initializeBilliardsViewers(root = document) {
  const setTableDetailVisible = (svg, selector, visible) => {
    svg.querySelectorAll(selector).forEach((element) => {
      element.style.display = visible ? '' : 'none';
    });
  };
  const applyTableDetailMode = (svg, mode) => {
    if (!svg) return;
    const resolvedMode = ['full', 'flat', 'cloth', 'rail'].includes(mode) ? mode : 'full';
    svg.dataset.tableDetail = resolvedMode;
    setTableDetailVisible(svg, tableDetailAllSelector, true);
    if (resolvedMode === 'flat') {
      setTableDetailVisible(svg, `${tableDetailGroups.clothTexture}, ${tableDetailGroups.railTexture}, ${tableDetailGroups.pocketTexture}`, false);
    } else if (resolvedMode === 'cloth') {
      setTableDetailVisible(svg, `${tableDetailGroups.rail}, ${tableDetailGroups.cushions}, ${tableDetailGroups.pockets}, ${tableDetailGroups.diamonds}`, false);
    } else if (resolvedMode === 'rail') {
      setTableDetailVisible(svg, tableDetailGroups.cloth, false);
    }
  };
  const globalTableDetail = document.querySelector('[data-global-table-detail]');
  const tableDetailModeForViewer = (viewer) => {
    const local = viewer.querySelector('[data-table-detail]')?.value;
    return local && local !== 'global' ? local : (globalTableDetail?.value ?? 'full');
  };
  const applyTableDetailToViewer = (viewer) => applyTableDetailMode(viewer.querySelector('svg'), tableDetailModeForViewer(viewer));
  const scenarioCards = Array.from(document.querySelectorAll('[data-scenario-card]'));
  const scenarioTocLinks = new Map(Array.from(document.querySelectorAll('[data-scenario-toc-link]')).map((link) => [link.getAttribute('href'), link]));
  const scenarioSearchInput = document.querySelector('[data-scenario-filter-search]');
  const scenarioSpeedFilter = document.querySelector('[data-scenario-filter-speed]');
  const scenarioEventFilter = document.querySelector('[data-scenario-filter-events]');
  const scenarioPlaybackFilter = document.querySelector('[data-scenario-filter-playback]');
  const scenarioFilterCount = document.querySelector('[data-scenario-filter-count]');
  const normalizeScenarioSearch = (value) => String(value ?? '').trim().toLowerCase();
  const applyScenarioFilters = () => {
    const query = normalizeScenarioSearch(scenarioSearchInput?.value);
    const speed = scenarioSpeedFilter?.value ?? '';
    const events = scenarioEventFilter?.value ?? '';
    const playback = scenarioPlaybackFilter?.value ?? '';
    let visibleCount = 0;
    scenarioCards.forEach((card) => {
      const eventCount = Number(card.dataset.scenarioEventCount ?? '0');
      const eventsMatch = !events
        || (events === 'any' ? eventCount > 0 : card.dataset.scenarioEvents === events);
      const visible = (!query || (card.dataset.scenarioSearch ?? '').includes(query))
        && (!speed || card.dataset.scenarioSpeedBand === speed)
        && eventsMatch
        && (!playback || card.dataset.scenarioPlayback === playback);
      card.hidden = !visible;
      scenarioTocLinks.get(`#${card.id}`)?.toggleAttribute('hidden', !visible);
      if (visible) visibleCount += 1;
    });
    if (scenarioFilterCount) {
      scenarioFilterCount.textContent = `Showing ${visibleCount} of ${scenarioCards.length} scenarios`;
      scenarioFilterCount.dataset.filtered = String(visibleCount !== scenarioCards.length);
    }
  };
  scenarioSearchInput?.addEventListener('input', applyScenarioFilters);
  scenarioSpeedFilter?.addEventListener('change', applyScenarioFilters);
  scenarioEventFilter?.addEventListener('change', applyScenarioFilters);
  scenarioPlaybackFilter?.addEventListener('change', applyScenarioFilters);
  document.querySelector('[data-scenario-filter-reset]')?.addEventListener('click', () => {
    if (scenarioSearchInput) scenarioSearchInput.value = '';
    if (scenarioSpeedFilter) scenarioSpeedFilter.value = '';
    if (scenarioEventFilter) scenarioEventFilter.value = '';
    if (scenarioPlaybackFilter) scenarioPlaybackFilter.value = '';
    applyScenarioFilters();
  });
  globalTableDetail?.addEventListener('change', () => {
    document.querySelectorAll('[data-viewer]').forEach(applyTableDetailToViewer);
  });
  applyScenarioFilters();
  const viewerRoots = [];
    if (root?.matches?.('[data-viewer]')) viewerRoots.push(root);
    root?.querySelectorAll?.('[data-viewer]')?.forEach((viewer) => viewerRoots.push(viewer));
    viewerRoots.forEach((viewer) => {
    hydrateViewerControls(viewer);
    const svg = viewer.querySelector('svg');
    if (!svg || !svg.viewBox || !svg.viewBox.baseVal) return;
    const base = svg.viewBox.baseVal;
    let box = { x: base.x, y: base.y, width: base.width, height: base.height };
    const apply = () => svg.setAttribute('viewBox', `${box.x} ${box.y} ${box.width} ${box.height}`);
    const zoom = (factor, cx = box.x + box.width / 2, cy = box.y + box.height / 2) => {
      const nextWidth = box.width * factor;
      const nextHeight = box.height * factor;
      const rx = (cx - box.x) / box.width;
      const ry = (cy - box.y) / box.height;
      box = { x: cx - nextWidth * rx, y: cy - nextHeight * ry, width: nextWidth, height: nextHeight };
      apply();
    };
    viewer.querySelectorAll('[data-zoom]').forEach((button) => {
      button.addEventListener('click', () => {
        const action = button.dataset.zoom;
        if (action === 'in') zoom(0.8);
        if (action === 'out') zoom(1.25);
        if (action === 'reset') { box = { x: base.x, y: base.y, width: base.width, height: base.height }; apply(); }
      });
    });
    const tableDetailSelect = viewer.querySelector('[data-table-detail]');
    if (tableDetailSelect) {
      tableDetailSelect.addEventListener('change', () => applyTableDetailToViewer(viewer));
    }
    applyTableDetailToViewer(viewer);
    viewer.querySelectorAll('[data-layer-toggle]').forEach((input) => {
      input.addEventListener('change', () => {
        svg.querySelectorAll(`[data-layer="${input.dataset.layerToggle}"]`).forEach((layer) => {
          layer.style.display = input.checked ? '' : 'none';
        });
      });
    });
    const card = viewer.closest('.card');
    const eventTitles = new Map(Array.from(card?.querySelectorAll('[data-event-label]') ?? []).map((row) => [row.getAttribute('data-event-label'), row.getAttribute('data-event-title')]));
    svg.querySelectorAll('.event-marker[data-event-label]').forEach((marker) => {
      const eventLabel = marker.getAttribute('data-event-label');
      const titleText = eventTitles.get(eventLabel);
      if (!titleText) return;
      marker.setAttribute('tabindex', '0');
      marker.setAttribute('aria-label', titleText);
      marker.classList.add('event-marker-tooltip');
      if (!marker.querySelector('title')) {
        const title = document.createElementNS('http://www.w3.org/2000/svg', 'title');
        title.textContent = titleText;
        marker.appendChild(title);
      } else {
        marker.querySelector('title').textContent = titleText;
      }
    });
    const playbackPanel = viewer.querySelector('[data-playback]');
    if (playbackPanel) {
      const playbackDataElement = playbackPanel.querySelector('[data-playback-data]');
      let playback = null;
      const normalizePlayback = (data) => {
        const normalizeEvent = (event) => Array.isArray(event)
          ? { label: String(event[0] ?? ''), time: Number(event[1]), summary: String(event[2] ?? '') }
          : { label: String(event?.label ?? ''), time: Number(event?.time), summary: String(event?.summary ?? '') };
        const normalizeVisual = (ball) => Array.isArray(ball)
          ? { id: String(ball[0] ?? ''), fill: String(ball[1] ?? ''), label: ball[2] == null ? null : String(ball[2]), radius: Number(ball[3]), radiusInches: Number(ball[4]) }
          : ball;
        const normalizeFrameBall = (ball) => {
          if (!Array.isArray(ball)) return ball;
          if (ball.length >= 10) {
            const vx = Number(ball[4]);
            const vy = Number(ball[5]);
            return { id: String(ball[0] ?? ''), x: Number(ball[1]), y: Number(ball[2]), heightInches: Number(ball[3]), vx, vy, vz: Number(ball[6]), wx: Number(ball[7]), wy: Number(ball[8]), wz: Number(ball[9]), speed: Math.hypot(vx, vy) };
          }
          const vx = Number(ball[3]);
          const vy = Number(ball[4]);
          return { id: String(ball[0] ?? ''), x: Number(ball[1]), y: Number(ball[2]), heightInches: 0, vz: 0, speed: Math.hypot(vx, vy), vx, vy, wx: Number(ball[5]), wy: Number(ball[6]), wz: Number(ball[7]) };
        };
        const normalizeFrame = (frame) => Array.isArray(frame)
          ? { time: Number(frame[0]), balls: Array.isArray(frame[1]) ? frame[1].map(normalizeFrameBall) : [] }
          : { ...frame, time: Number(frame?.time), balls: Array.isArray(frame?.balls) ? frame.balls.map(normalizeFrameBall) : [] };
        const balls = Array.isArray(data?.balls) ? data.balls.map(normalizeVisual) : [];
        const visualById = new Map(balls.map((ball) => [ball.id, ball]));
        const frames = Array.isArray(data?.frames) ? data.frames.map(normalizeFrame) : [];
        frames.forEach((frame) => {
          frame.balls.forEach((ball) => {
            const visual = visualById.get(ball.id);
            if (visual && !Number.isFinite(Number(ball.ballRadiusInches))) {
              ball.ballRadiusInches = visual.radiusInches;
            }
          });
        });
        return {
          duration: Number(data?.duration) || 0,
          events: Array.isArray(data?.events) ? data.events.map(normalizeEvent) : [],
          balls,
          frames,
        };
      };
      try {
        playback = normalizePlayback(JSON.parse(playbackDataElement?.textContent ?? ''));
      } catch (_) {
        playback = null;
      }
      if (playback && Array.isArray(playback.frames) && playback.frames.length > 0) {
        hydratePlaybackControls(playbackPanel, playback);
      }
      const slider = playbackPanel.querySelector('[data-playback-slider]');
      const speedSlider = playbackPanel.querySelector('[data-playback-speed]');
      const speedLabel = playbackPanel.querySelector('[data-playback-speed-label]');
      const traceToggle = playbackPanel.querySelector('[data-playback-trace]');
      const timeLabel = playbackPanel.querySelector('[data-playback-time]');
      const eventTicker = playbackPanel.querySelector('[data-playback-event]');
      const playButton = playbackPanel.querySelector('[data-playback-play]');
      const resetPlaybackButton = playbackPanel.querySelector('[data-playback-reset]');
      const nextEventButton = playbackPanel.querySelector('[data-playback-next-event]');
      if (playback && Array.isArray(playback.frames) && playback.frames.length > 0 && slider) {
        const ns = 'http://www.w3.org/2000/svg';
        const ballLayer = svg.querySelector('[data-layer="balls"], [data-layer="static-balls"]');
        if (ballLayer) {
          ballLayer.style.display = 'none';
          ballLayer.setAttribute('data-layer', 'static-balls');
        }
        svg.querySelectorAll('.diagram-layer .ball-spin-glyph').forEach((glyph) => {
          glyph.style.display = 'none';
        });
        const traceElements = Array.from(svg.querySelectorAll('.smooth-polyline, .heading-chevron'));
        const setTracePathsVisible = (visible) => {
          traceElements.forEach((element) => {
            element.style.display = visible ? '' : 'none';
          });
        };
        if (traceToggle) {
          traceToggle.addEventListener('change', () => setTracePathsVisible(traceToggle.checked));
          setTracePathsVisible(traceToggle.checked);
        }
        const playbackLayer = document.createElementNS(ns, 'g');
        playbackLayer.setAttribute('class', 'playback-layer playback-balls');
        playbackLayer.setAttribute('data-layer', 'balls');
        if (ballLayer && ballLayer.parentNode) {
          ballLayer.parentNode.insertBefore(playbackLayer, ballLayer.nextSibling);
        } else {
          svg.appendChild(playbackLayer);
        }
        const visuals = new Map((playback.balls ?? []).map((ball) => [ball.id, ball]));
        const events = Array.isArray(playback.events)
          ? playback.events
              .map((event) => ({
                label: String(event.label ?? ''),
                time: Number(event.time),
                summary: String(event.summary ?? ''),
              }))
              .filter((event) => event.label && Number.isFinite(event.time))
              .sort((a, b) => a.time - b.time)
          : [];
        const eventRows = new Map(Array.from(card?.querySelectorAll('.event-list [data-event-label]') ?? []).map((row) => [row.getAttribute('data-event-label'), row]));
        const eventHitWindow = 0.005;
        const clampFrame = (value) => Math.max(0, Math.min(playback.frames.length - 1, Number(value) || 0));
        const frameTime = (frameIndex) => Number(playback.frames[clampFrame(frameIndex)]?.time) || 0;
        const formatPlaybackTime = (time) => (Number(time) || 0).toFixed(3);
        const minPlaybackSpeed = 1 / 16;
        const maxPlaybackSpeed = 1;
        const playbackSpeed = () => {
          const raw = Number(speedSlider?.value);
          if (!Number.isFinite(raw)) return maxPlaybackSpeed;
          return Math.max(minPlaybackSpeed, Math.min(maxPlaybackSpeed, raw));
        };
        const formatPlaybackSpeed = (speed) => {
          const inverse = Math.round(1 / speed);
          if (Math.abs(speed - 1) <= 1e-9) return '1x';
          if (inverse > 1 && Math.abs(speed - 1 / inverse) <= 1e-6) return `1/${inverse}x`;
          return `${speed.toFixed(2)}x`;
        };
        const updateSpeedLabel = () => {
          if (speedLabel) speedLabel.textContent = formatPlaybackSpeed(playbackSpeed());
        };
        const nextEventAfter = (time) => events.find((event) => event.time > time + eventHitWindow);
        const updateEventTicker = (time) => {
          eventRows.forEach((row) => row.classList.remove('event-current'));
          if (!eventTicker) return;
          if (events.length === 0) {
            eventTicker.textContent = 'no logged events';
            eventTicker.dataset.eventState = 'none';
            return;
          }
          const hit = events.find((event) => Math.abs(event.time - time) <= eventHitWindow);
          if (hit) {
            eventTicker.textContent = `event ${hit.label} @ t=${formatPlaybackTime(hit.time)}s: ${hit.summary}`;
            eventTicker.dataset.eventState = 'hit';
            eventRows.get(hit.label)?.classList.add('event-current');
            return;
          }
          const next = nextEventAfter(time);
          if (next) {
            eventTicker.textContent = `next ${next.label} in ${Math.max(0, next.time - time).toFixed(3)}s: ${next.summary}`;
            eventTicker.dataset.eventState = 'next';
            return;
          }
          const last = events.slice().reverse().find((event) => event.time <= time + eventHitWindow);
          if (last) {
            eventTicker.textContent = `last ${last.label} @ t=${formatPlaybackTime(last.time)}s: ${last.summary}`;
            eventTicker.dataset.eventState = 'done';
            eventRows.get(last.label)?.classList.add('event-current');
          } else {
            eventTicker.textContent = 'before first event';
            eventTicker.dataset.eventState = 'before';
          }
        };
        const frameBallMap = (frame) => new Map((frame?.balls ?? []).map((ball) => [ball.id, ball]));
        const headingForBall = (index, ball) => {
          const previous = frameBallMap(playback.frames[Math.max(0, index - 1)]).get(ball.id);
          const next = frameBallMap(playback.frames[Math.min(playback.frames.length - 1, index + 1)]).get(ball.id);
          const dx = next && (Math.abs(next.x - ball.x) > 0.01 || Math.abs(next.y - ball.y) > 0.01)
            ? next.x - ball.x
            : previous ? ball.x - previous.x : 0;
          const dy = next && (Math.abs(next.x - ball.x) > 0.01 || Math.abs(next.y - ball.y) > 0.01)
            ? next.y - ball.y
            : previous ? ball.y - previous.y : 0;
          const length = Math.hypot(dx, dy);
          if (length <= 0.01) return null;
          return { dx: dx / length, dy: dy / length };
        };
        const appendCircle = (className, cx, cy, radius, fill, opacity, stroke = 'none', strokeWidth = '0') => {
          const circle = document.createElementNS(ns, 'circle');
          circle.setAttribute('class', className);
          circle.setAttribute('cx', cx.toFixed(3));
          circle.setAttribute('cy', cy.toFixed(3));
          circle.setAttribute('r', radius.toFixed(3));
          circle.setAttribute('fill', fill);
          circle.setAttribute('fill-opacity', opacity.toFixed(3));
          circle.setAttribute('stroke', stroke);
          circle.setAttribute('stroke-width', strokeWidth);
          playbackLayer.appendChild(circle);
        };
        const spinStun = 1e-6;
        const spinGrey = [0x7f, 0x85, 0x8c];
        const spinGreen = [0x2d, 0xa4, 0x4e];
        const spinBlue = [0x09, 0x6b, 0xd8];
        const spinOrange = [0xfb, 0x85, 0x1e];
        const spinAmber = [0xbf, 0x87, 0x00];
        const spinViolet = [0x8b, 0x5c, 0xf6];
        const finiteNumber = (value) => {
          const number = Number(value);
          return Number.isFinite(number) ? number : 0;
        };
        const hexColor = (color) => `#${color.map((channel) => Math.round(channel).toString(16).padStart(2, '0')).join('')}`;
        const mixColor = (start, end, t) => start.map((channel, index) => channel + (end[index] - channel) * Math.max(0, Math.min(1, t)));
        const svgNode = (parent, name, attrs = {}) => {
          const element = document.createElementNS(ns, name);
          Object.entries(attrs).forEach(([key, value]) => element.setAttribute(key, String(value)));
          parent.appendChild(element);
          return element;
        };
        const spinMetrics = (ball) => {
          const vx = finiteNumber(ball.vx);
          const vy = finiteNumber(ball.vy);
          const wx = finiteNumber(ball.wx);
          const wy = finiteNumber(ball.wy);
          const wz = finiteNumber(ball.wz);
          const planar = Math.hypot(wx, wy);
          const total = Math.hypot(planar, wz);
          const speed = Math.hypot(vx, vy);
          const radiusInches = Math.max(spinStun, finiteNumber(ball.ballRadiusInches) || 1.125);
          const suppliedRollingTarget = Math.max(0, finiteNumber(ball.rollingTarget));
          const rollingTarget = suppliedRollingTarget > spinStun ? suppliedRollingTarget : speed / radiusInches;
          const rollRatio = rollingTarget > spinStun ? planar / rollingTarget : 0;
          const rollVx = radiusInches * wy;
          const rollVy = -radiusInches * wx;
          const rollSpeed = Math.hypot(rollVx, rollVy);
          const rollSlip = Math.hypot(vx - rollVx, vy - rollVy);
          const rollAlignment = speed > spinStun && rollSpeed > spinStun
            ? Math.max(-1, Math.min(1, (vx * rollVx + vy * rollVy) / (speed * rollSpeed)))
            : 0;
          const angle = rollSpeed > spinStun ? Math.atan2(-rollVy, rollVx) * 180 / Math.PI : 0;
          const rollingSlipLimit = Math.max(speed * 0.12, 0.75);
          const isRolling = speed > spinStun && planar > spinStun && rollSlip <= rollingSlipLimit;
          const hasProminentSide = Math.abs(wz) > Math.max(planar, rollingTarget) * 0.25;
          let kind = 'stun';
          if (total > spinStun && isRolling && hasProminentSide) {
            kind = 'rolling-english';
          } else if (total > spinStun && isRolling) {
            kind = 'rolling';
          } else if (total > spinStun && rollAlignment <= -0.5) {
            kind = 'draw';
          } else if (total > spinStun && rollAlignment >= 0.5 && rollRatio > 1.15) {
            kind = 'follow';
          } else if (total > spinStun && Math.abs(wz) >= planar) {
            kind = 'english';
          } else if (total > spinStun) {
            kind = 'spin';
          }
          const planarColor = kind === 'stun' || kind === 'english'
            ? hexColor(spinGrey)
            : kind === 'rolling' || kind === 'rolling-english'
              ? hexColor(spinGreen)
              : kind === 'draw'
                ? hexColor(spinOrange)
                : kind === 'follow'
                  ? hexColor(spinBlue)
                  : hexColor(mixColor(spinGrey, spinAmber, planar / Math.max(rollingTarget, 120)));
          const zColor = hexColor(spinViolet);
          return { vx, vy, wx, wy, wz, planar, total, rollingTarget, rollRatio, rollAlignment, rollSlip, angle, kind, planarColor, zColor };
        };
        const appendSpinGlyph = (ball, radius) => {
          const metrics = spinMetrics(ball);
          const glyphRadius = Math.max(8.5, Math.min(13.0, radius * 0.58));
          const badgeOffset = radius * 0.72;
          const strokeWidth = Math.max(2.4, Math.min(4.0, radius * 0.135));
          const titleText = `spin: v=(${metrics.vx.toFixed(1)}, ${metrics.vy.toFixed(1)}) ips; omega=(${metrics.wx.toFixed(1)}, ${metrics.wy.toFixed(1)}, ${metrics.wz.toFixed(1)}) rad/s; roll slip=${metrics.rollSlip.toFixed(1)} ips; roll ratio=${metrics.rollRatio.toFixed(2)}; side=${metrics.wz.toFixed(1)} rad/s`;
          const group = svgNode(playbackLayer, 'g', {
            class: 'playback-spin-glyph ball-spin-glyph',
            role: 'img',
            'aria-label': titleText,
            transform: `translate(${(finiteNumber(ball.x) + badgeOffset).toFixed(3)} ${(finiteNumber(ball.y) - badgeOffset).toFixed(3)})`,
            'data-spin-kind': metrics.kind,
            'data-spin-angle-deg': metrics.angle.toFixed(3),
            'data-spin-rps': metrics.total.toFixed(3),
            'data-spin-planar-rps': metrics.planar.toFixed(3),
            'data-spin-z-rps': metrics.wz.toFixed(3),
            'data-spin-roll-ratio': metrics.rollRatio.toFixed(3),
            'data-spin-roll-alignment': metrics.rollAlignment.toFixed(3),
            'data-spin-slip-ips': metrics.rollSlip.toFixed(3),
            'data-spin-vx': metrics.vx.toFixed(3),
            'data-spin-vy': metrics.vy.toFixed(3),
            'data-spin-wx': metrics.wx.toFixed(3),
            'data-spin-wy': metrics.wy.toFixed(3),
            'data-spin-wz': metrics.wz.toFixed(3),
          });
          const title = svgNode(group, 'title');
          title.textContent = titleText;
          svgNode(group, 'circle', {
            class: 'ball-spin-backplate',
            r: glyphRadius.toFixed(3),
            'stroke-width': (strokeWidth * 0.75).toFixed(3),
          });
          if (metrics.total <= spinStun) {
            const arm = glyphRadius * 0.48;
            const xPath = `M ${(-arm).toFixed(3)} ${(-arm).toFixed(3)} L ${arm.toFixed(3)} ${arm.toFixed(3)} M ${arm.toFixed(3)} ${(-arm).toFixed(3)} L ${(-arm).toFixed(3)} ${arm.toFixed(3)}`;
            svgNode(group, 'path', {
              class: 'ball-spin-stun-x-halo',
              d: xPath,
              'stroke-width': (strokeWidth * 2.7).toFixed(3),
            });
            svgNode(group, 'path', {
              class: 'ball-spin-stun-x-mark',
              d: xPath,
              'stroke-width': (strokeWidth * 1.35).toFixed(3),
            });
            return;
          }
          const rotor = svgNode(group, 'g', { transform: `rotate(${metrics.angle.toFixed(3)})` });
          if (metrics.planar > spinStun) {
            const tail = -glyphRadius * 0.70;
            const tip = glyphRadius * 0.74;
            const head = glyphRadius * 0.36;
            const base = tip - head;
            svgNode(rotor, 'path', {
              class: 'ball-spin-vector-halo',
              d: `M ${tail.toFixed(3)} 0 L ${base.toFixed(3)} 0`,
              'stroke-width': (strokeWidth * 2.65).toFixed(3),
            });
            svgNode(rotor, 'path', {
              class: 'ball-spin-vector',
              d: `M ${tail.toFixed(3)} 0 L ${base.toFixed(3)} 0`,
              stroke: metrics.planarColor,
              'stroke-opacity': '.98',
              'stroke-width': (strokeWidth * 1.28).toFixed(3),
            });
            svgNode(rotor, 'path', {
              class: 'ball-spin-arrowhead',
              d: `M ${tip.toFixed(3)} 0 L ${base.toFixed(3)} ${(-head * 0.70).toFixed(3)} L ${base.toFixed(3)} ${(head * 0.70).toFixed(3)} Z`,
              fill: metrics.planarColor,
              'fill-opacity': '.98',
              'stroke-width': (strokeWidth * 0.55).toFixed(3),
            });
          }
          if (Math.abs(metrics.wz) > spinStun) {
            const arc = glyphRadius * 0.82;
            const zOpacity = Math.max(0.66, Math.min(1, Math.abs(metrics.wz) / metrics.total));
            const zGroup = svgNode(group, 'g', { transform: `scale(${metrics.wz >= 0 ? '1.0' : '-1.0'} 1)` });
            svgNode(zGroup, 'path', {
              class: 'ball-spin-z-halo',
              d: `M ${(-arc).toFixed(3)} ${(-arc * 0.42).toFixed(3)} A ${arc.toFixed(3)} ${arc.toFixed(3)} 0 1 1 ${arc.toFixed(3)} ${(arc * 0.42).toFixed(3)}`,
              'stroke-width': (strokeWidth * 2.25).toFixed(3),
            });
            svgNode(zGroup, 'path', {
              class: 'ball-spin-z',
              d: `M ${(-arc).toFixed(3)} ${(-arc * 0.42).toFixed(3)} A ${arc.toFixed(3)} ${arc.toFixed(3)} 0 1 1 ${arc.toFixed(3)} ${(arc * 0.42).toFixed(3)}`,
              stroke: metrics.zColor,
              'stroke-opacity': zOpacity.toFixed(3),
              'stroke-width': (strokeWidth * 1.18).toFixed(3),
            });
            const head = glyphRadius * 0.30;
            svgNode(zGroup, 'path', {
              class: 'ball-spin-z-head',
              d: `M ${arc.toFixed(3)} ${(arc * 0.42).toFixed(3)} L ${(arc - head * 0.72).toFixed(3)} ${(arc * 0.42 - head * 0.78).toFixed(3)} L ${(arc - head * 0.12).toFixed(3)} ${(arc * 0.42 + head * 0.90).toFixed(3)} Z`,
              fill: metrics.zColor,
              'fill-opacity': zOpacity.toFixed(3),
              'stroke-width': (strokeWidth * 0.55).toFixed(3),
            });
          }
        };
        const paintPlayback = (frameIndex) => {
          const index = clampFrame(frameIndex);
          const frame = playback.frames[index];
          const time = Number(frame.time) || 0;
          playbackLayer.replaceChildren();
          for (const ball of frame.balls ?? []) {
            const visual = visuals.get(ball.id) ?? {};
            const radius = Number(visual.radius) || 12;
            const radiusInches = Math.max(spinStun, Number(visual.radiusInches) || Number(ball.ballRadiusInches) || 1.125);
            const liftPx = Math.max(0, finiteNumber(ball.heightInches)) * radius / radiusInches;
            const displayX = finiteNumber(ball.x) - liftPx;
            const displayY = finiteNumber(ball.y);
            const liftRatio = Math.max(0, Math.min(1.6, liftPx / Math.max(radius, 1)));
            appendCircle('playback-ball-shadow', finiteNumber(ball.x) + radius * (0.12 + 0.11 * liftRatio), finiteNumber(ball.y) + radius * (0.18 + 0.10 * liftRatio), radius * (1.02 + 0.22 * liftRatio), '#000', Math.max(0.08, 0.25 - 0.08 * liftRatio));
            appendCircle('playback-ball', displayX, displayY, radius, visual.fill || '#ffffff', 1, '#111', '1.25');
            const heading = headingForBall(index, ball);
            const speed = Math.max(0, Number(ball.speed) || 0);
            if (heading && speed > 0.05) {
              const lineLength = radius * (1.18 + Math.min(speed, 160) / 220);
              const halfLength = lineLength / 2;
              const x1 = displayX - heading.dx * halfLength;
              const y1 = displayY - heading.dy * halfLength;
              const x2 = displayX + heading.dx * halfLength;
              const y2 = displayY + heading.dy * halfLength;
              const width = Math.max(0.5, Math.min(4.8, 0.5 + speed / 55));
              const opacity = Math.max(0.18, Math.min(1, speed / 80));
              const line = document.createElementNS(ns, 'line');
              line.setAttribute('class', 'playback-heading');
              line.setAttribute('x1', x1.toFixed(3));
              line.setAttribute('y1', y1.toFixed(3));
              line.setAttribute('x2', x2.toFixed(3));
              line.setAttribute('y2', y2.toFixed(3));
              line.setAttribute('stroke-width', width.toFixed(3));
              line.setAttribute('stroke-opacity', opacity.toFixed(3));
              playbackLayer.appendChild(line);
            }
            if (visual.label) {
              const label = document.createElementNS(ns, 'text');
              label.setAttribute('class', 'playback-ball-label');
              label.setAttribute('x', displayX.toFixed(3));
              label.setAttribute('y', displayY.toFixed(3));
              label.setAttribute('transform', `rotate(-90 ${displayX.toFixed(3)} ${displayY.toFixed(3)})`);
              label.textContent = visual.label;
              playbackLayer.appendChild(label);
            }
            appendSpinGlyph({ ...ball, x: displayX, y: displayY }, radius);
          }
          slider.value = String(index);
          if (timeLabel) timeLabel.textContent = `t=${formatPlaybackTime(time)}s`;
          updateEventTicker(time);
        };
        let playing = false;
        let animationId = null;
        let playStartedAt = 0;
        let playStartTime = 0;
        let playTargetTime = null;
        const setPlayButtonState = (isPlaying) => {
          if (!playButton) return;
          playButton.textContent = isPlaying ? '⏸' : '▶';
          const label = isPlaying ? 'Pause' : 'Play';
          playButton.setAttribute('aria-label', label);
          playButton.setAttribute('title', label);
        };
        const stopPlayback = () => {
          playing = false;
          if (animationId !== null) cancelAnimationFrame(animationId);
          animationId = null;
          playTargetTime = null;
          setPlayButtonState(false);
        };
        const nearestFrameForTime = (time) => {
          let bestIndex = 0;
          let bestDistance = Infinity;
          playback.frames.forEach((frame, index) => {
            const distance = Math.abs((Number(frame.time) || 0) - time);
            if (distance < bestDistance) {
              bestIndex = index;
              bestDistance = distance;
            }
          });
          return bestIndex;
        };
        const startPlayback = (startIndex, targetTime = null) => {
          stopPlayback();
          const duration = Math.max(0, Number(playback.duration) || 0);
          const boundedTarget = Number.isFinite(targetTime) ? Math.max(0, Math.min(duration, targetTime)) : null;
          playStartTime = frameTime(startIndex);
          if (boundedTarget !== null && boundedTarget <= playStartTime + eventHitWindow) {
            paintPlayback(nearestFrameForTime(boundedTarget));
            return;
          }
          playing = true;
          playTargetTime = boundedTarget;
          setPlayButtonState(true);
          playStartedAt = performance.now();
          paintPlayback(startIndex);
          animationId = requestAnimationFrame(tick);
        };
        const tick = (now) => {
          if (!viewer.isConnected || !playbackLayer.isConnected) {
            stopPlayback();
            return;
          }
          if (!playing) return;
          const duration = Math.max(0, Number(playback.duration) || 0);
          const targetTime = playTargetTime === null ? duration : playTargetTime;
          const elapsed = ((now - playStartedAt) / 1000) * playbackSpeed();
          const time = playStartTime + elapsed;
          if (duration > 0 && time >= targetTime - eventHitWindow) {
            paintPlayback(nearestFrameForTime(targetTime));
            stopPlayback();
            return;
          }
          paintPlayback(nearestFrameForTime(time));
          animationId = requestAnimationFrame(tick);
        };
        slider.addEventListener('input', () => {
          stopPlayback();
          paintPlayback(slider.value);
        });
        if (speedSlider) {
          speedSlider.addEventListener('input', () => {
            updateSpeedLabel();
            if (playing) {
              playStartTime = frameTime(slider.value);
              playStartedAt = performance.now();
            }
          });
        }
        if (resetPlaybackButton) {
          resetPlaybackButton.addEventListener('click', () => {
            stopPlayback();
            paintPlayback(0);
          });
        }
        viewer.querySelectorAll('[data-playback-step]').forEach((button) => {
          button.addEventListener('click', () => {
            stopPlayback();
            paintPlayback(clampFrame(slider.value) + Number(button.dataset.playbackStep));
          });
        });
        if (nextEventButton) {
          nextEventButton.addEventListener('click', () => {
            const next = nextEventAfter(frameTime(slider.value));
            if (!next) {
              stopPlayback();
              paintPlayback(playback.frames.length - 1);
              return;
            }
            startPlayback(clampFrame(slider.value), next.time);
          });
        }
        if (playButton) {
          playButton.addEventListener('click', () => {
            if (playing) {
              stopPlayback();
              return;
            }
            let startIndex = clampFrame(slider.value);
            if (startIndex >= playback.frames.length - 1) startIndex = 0;
            startPlayback(startIndex);
          });
        }
        updateSpeedLabel();
        paintPlayback(playback.frames.length - 1);
      }
    }
    svg.addEventListener('wheel', (event) => {
      event.preventDefault();
      const rect = svg.getBoundingClientRect();
      const cx = box.x + ((event.clientX - rect.left) / rect.width) * box.width;
      const cy = box.y + ((event.clientY - rect.top) / rect.height) * box.height;
      zoom(event.deltaY < 0 ? 0.9 : 1.1, cx, cy);
    }, { passive: false });
    let drag = null;
    svg.addEventListener('pointerdown', (event) => {
      svg.setPointerCapture(event.pointerId);
      svg.classList.add('dragging');
      drag = { x: event.clientX, y: event.clientY, box: { ...box } };
    });
    svg.addEventListener('pointermove', (event) => {
      if (!drag) return;
      const rect = svg.getBoundingClientRect();
      box.x = drag.box.x - ((event.clientX - drag.x) / rect.width) * drag.box.width;
      box.y = drag.box.y - ((event.clientY - drag.y) / rect.height) * drag.box.height;
      apply();
    });
    const stopDrag = () => { drag = null; svg.classList.remove('dragging'); };
    svg.addEventListener('pointerup', stopDrag);
    svg.addEventListener('pointercancel', stopDrag);
  });
  }

  window.BilliardsReportViewer = { initialize: initializeBilliardsViewers, initializeBilliardsViewers, viewerControlsHtml, playbackPanelHtml };

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => initializeBilliardsViewers(document), { once: true });
  } else {
    initializeBilliardsViewers(document);
  }
})();
