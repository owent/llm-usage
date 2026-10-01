/** Statistics use the selected IANA timezone, independently of the OS clock. */
export function calendarDay(now: Date, timezone: string): string {
  const parts = new Intl.DateTimeFormat('en-CA', {
    timeZone: timezone, year: 'numeric', month: '2-digit', day: '2-digit',
  }).formatToParts(now);
  const part = (name: string) => parts.find((p) => p.type === name)?.value;
  return `${part('year')}-${part('month')}-${part('day')}`;
}

/** Offset calendar dates, without assuming a local day lasts exactly 24 hours. */
export function offsetDay(day: string, days: number): string {
  const date = new Date(`${day}T12:00:00Z`);
  date.setUTCDate(date.getUTCDate() + days);
  return date.toISOString().slice(0, 10);
}

/** The activity calendar is a full calendar year, independently of chart range. */
export function calendarYearRange(day: string, selectedYear?: number) {
  const year = selectedYear ?? Number(day.slice(0, 4));
  return { year, first_day: `${year}-01-01`, last_day: `${year}-12-31` };
}
