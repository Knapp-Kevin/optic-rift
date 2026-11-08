import { useMemo, useState } from "react";
import { creativeJourneys, journeyAreas, journeyStateText, type JourneyArea, type JourneyState } from "./creativeJourneys";
import type { View } from "./creativeNavigation";

type Props = { onNavigate: (view: View) => void };

export default function CreativeWorkflowExplorer({ onNavigate }: Props) {
  const [filter, setFilter] = useState<"all" | JourneyArea>("all");
  const [availability, setAvailability] = useState<"all" | JourneyState>("all");
  const [query, setQuery] = useState("");

  const journeys = useMemo(() => {
    const term = query.trim().toLocaleLowerCase();
    return creativeJourneys.filter((journey) =>
      (filter === "all" || journey.area === filter) &&
      (availability === "all" || journey.state === availability) &&
      (!term || [journey.title, journey.outcome, journey.current].some((text) => text.toLocaleLowerCase().includes(term))));
  }, [filter, availability, query]);

  return <main className="video-compose-page workflow-explorer-page">
    <div className="page-eyebrow">OPTIC RIFT · CREATIVE WORKFLOWS</div>
    <h1>What do you want to make or improve?</h1>
    <p className="lead">Browse the journeys Optic Rift is designed to support. This page distinguishes what the
      development build can do today, what can only be planned, and what still needs engineering.
      It isn't a promise that a discovered model can run on this device.</p>
    <div className="video-compose-status">
      <strong>Development roadmap, not a release feature checklist</strong>
      <p>Photo, video, enhancement and short-film workflows belong in one app and Media Library.
        Audio creation and mixing remain outside Optic Rift's scope. Nothing on this page starts a GPU job.</p>
    </div>
    <div className="workflow-explorer-filters">
      <label>Creative area
        <select value={filter} onChange={(event) => setFilter(event.target.value as "all" | JourneyArea)}>
          <option value="all">All workflows</option>
          {journeyAreas.map((area) => <option value={area} key={area}>{area}</option>)}
        </select>
      </label>
      <label>Current state
        <select value={availability} onChange={(event) => setAvailability(event.target.value as "all" | JourneyState)}>
          <option value="all">All states</option>
          {Object.entries(journeyStateText).map(([state, name]) =>
            <option value={state} key={state}>{name}</option>)}
        </select>
      </label>
      <label>Search user journeys
        <input type="search" value={query} onChange={(event) => setQuery(event.target.value)}
          placeholder="Upscaling, animation, retouch, film…" />
      </label>
    </div>
    <p className="workflow-count" role="status">{journeys.length} of {creativeJourneys.length} user journeys</p>
    <div className="workflow-explorer-grid">
      {journeys.map((journey) => <article key={journey.id} className="workflow-journey-card">
        <div className="workflow-journey-meta"><span>{journey.area} · {journey.id}</span>
          <span className="workflow-state">{journeyStateText[journey.state]}</span></div>
        <h2>{journey.title}</h2>
        <p>{journey.outcome}</p>
        <dl><dt>Current reality</dt><dd>{journey.current}</dd>
          <dt>End-to-end acceptance</dt><dd>{journey.acceptance}</dd></dl>
        {journey.route ? <button type="button" className="secondary"
          onClick={() => onNavigate(journey.route!)}>
          {journey.state === "planning-only" ? "Open planning workspace" : "Open related workspace"}
        </button> : <p className="video-form-hint">No working destination yet. Implementation and qualification required.</p>}
      </article>)}
    </div>
    {journeys.length === 0 && <p className="session-library-empty">
      No journeys match those filters. Try a different creative area or search.
    </p>}
  </main>;
}
