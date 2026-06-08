import "./index.css";
import { Composition } from "remotion";
import {
  BrassClawArchitecture,
  TOTAL_DURATION,
} from "./BrassClawArchitecture";

export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition
        id="BrassClawArchitecture"
        component={BrassClawArchitecture}
        durationInFrames={TOTAL_DURATION}
        fps={30}
        width={1280}
        height={720}
      />
    </>
  );
};
