import { HeroCarousel } from "@/components/content/HeroCarousel";
import { ContentRow } from "@/components/content/ContentRow";
import { repository } from "@/lib/repository";
import { useAppStore } from "@/lib/store";
import type { Film, Series } from "@/types";
import "./HomePage.css";

export function HomePage() {
  const continueWatching = useAppStore((s) => s.continueWatching);

  const trending = repository.listTrending();
  const originals = repository.listOriginals();
  const liveNow = repository.listLiveStreams();
  const series = repository.listSeries();
  const films = repository.listFilms();
  const sciFi = repository.listByGenre("Sci-Fi");

  const heroItems = originals.slice(0, 4);

  const progressMap = Object.fromEntries(
    continueWatching.map((c) => [c.contentId, c.progressSec / c.durationSec]),
  );

  const continueItems = continueWatching
    .map((c) => repository.getByAnyId(c.contentId))
    .filter((x): x is Series | Film => x !== undefined && x.kind !== "live");

  return (
    <div className="ls-home">
      <HeroCarousel items={heroItems} />

      <div className="ls-home__rows">
        {continueItems.length > 0 && (
          <ContentRow
            kicker="01 / Your queue"
            title="Continue watching"
            items={continueItems}
            layout="landscape"
            progressById={progressMap}
            seeAllHref="/library"
          />
        )}

        <ContentRow
          kicker="02 / Right now"
          title="Live on LIFESTREAM"
          items={liveNow}
          layout="landscape"
          seeAllHref="/live"
        />

        <ContentRow
          kicker="03 / Trending"
          title="Everyone is watching"
          items={trending}
          layout="poster"
          seeAllHref="/browse"
        />

        <ContentRow
          kicker="04 / Originals"
          title="LIFESTREAM Originals"
          items={originals}
          layout="poster"
          seeAllHref="/browse/originals"
        />

        <ContentRow
          kicker="05 / Series"
          title="Series we're obsessed with"
          items={series}
          layout="poster"
          seeAllHref="/series"
        />

        <ContentRow
          kicker="06 / Films"
          title="Films worth the runtime"
          items={films}
          layout="poster"
          seeAllHref="/films"
        />

        <ContentRow
          kicker="07 / Genre"
          title="Hard science fiction"
          items={sciFi}
          layout="poster"
          seeAllHref="/category/sci-fi"
        />
      </div>
    </div>
  );
}
