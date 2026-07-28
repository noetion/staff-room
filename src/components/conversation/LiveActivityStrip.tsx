export type LiveActivityItem = { title: string; detail: string };

export function LiveActivityStrip({ activity }: { activity: LiveActivityItem[] }) {
  return (
    <section
      className="live-activity-strip"
      role="log"
      aria-label="Live provider activity"
      aria-live="off"
      aria-relevant="additions text"
    >
      <div className="live-activity-rows">
        {activity.slice(-6).map((item, index) => (
          <div className="live-activity-row" key={`${item.title}-${index}`}>
            <strong>{item.title}</strong>
            <span>{item.detail}</span>
          </div>
        ))}
      </div>
      <small className="live-activity-note">
        Provider-reported reasoning and tool activity only. Full output remains in the local run log.
      </small>
    </section>
  );
}
