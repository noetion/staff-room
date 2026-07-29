import { calendarDay, parseTimestamp } from "../../lib/format";

export interface DayDividerProps {
  date: string;
}

function localDay(value: Date) {
  return `${value.getFullYear()}-${value.getMonth()}-${value.getDate()}`;
}

function dayLabel(value: string) {
  const time = parseTimestamp(value);
  if (Number.isNaN(time)) return "EARLIER";
  const date = new Date(time);
  const today = new Date();
  const yesterday = new Date();
  yesterday.setDate(today.getDate() - 1);

  if (calendarDay(value) === localDay(today)) return "TODAY";
  if (calendarDay(value) === localDay(yesterday)) return "YESTERDAY";
  return new Intl.DateTimeFormat("en-GB", {
    weekday: "short",
    day: "2-digit",
    month: "short",
  }).format(date).replace(",", "").toLocaleUpperCase();
}

export function DayDivider({ date }: DayDividerProps) {
  return (
    <div className="day-divider" role="separator" aria-label={dayLabel(date)}>
      <span>{dayLabel(date)}</span>
    </div>
  );
}
