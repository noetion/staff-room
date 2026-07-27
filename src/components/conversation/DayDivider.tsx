export interface DayDividerProps {
  date: string;
}

function calendarDay(value: Date) {
  return `${value.getFullYear()}-${value.getMonth()}-${value.getDate()}`;
}

function dayLabel(value: string) {
  const date = new Date(value);
  const today = new Date();
  const yesterday = new Date();
  yesterday.setDate(today.getDate() - 1);

  if (calendarDay(date) === calendarDay(today)) return "TODAY";
  if (calendarDay(date) === calendarDay(yesterday)) return "YESTERDAY";
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
