import "./index.css";
import { Composition } from "remotion";
import {
  BrassClawArchitecture,
  TOTAL_DURATION,
} from "./BrassClawArchitecture";

// Video output dimensions (1080p wide-screen presentation format).
const VIDEO_WIDTH = 1280;
const VIDEO_HEIGHT = 720;

export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition
        id="BrassClawArchitecture"
        component={BrassClawArchitecture}
        durationInFrames={TOTAL_DURATION}
        fps={30}
        width={VIDEO_WIDTH}
        height={VIDEO_HEIGHT}
      />
    </>
  );
};
