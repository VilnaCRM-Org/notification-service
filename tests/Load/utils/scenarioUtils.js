import ScenariosBuilder from './scenariosBuilder.js';
import ThresholdsBuilder from './thresholdsBuilder.js';

export default class ScenarioUtils {
  constructor(utils, scenarioName) {
    this.utils = utils;
    this.config = utils.getConfig();
    this.smokeConfig = this.config.endpoints[scenarioName].smoke;
    this.averageConfig = this.config.endpoints[scenarioName].average;
    this.stressConfig = this.config.endpoints[scenarioName].stress;
    this.spikeConfig = this.config.endpoints[scenarioName].spike;
    this.delay = this.config.delayBetweenScenarios;
    // Start time of the next selected profile: each profile starts after the selected ones before it and the pause.
    this.nextStartTime = 0;
  }

  getOptions() {
    return {
      setupTimeout: '60s',
      teardownTimeout: '60s',
      scenarios: this.getScenarios(),
      thresholds: this.getThresholds(),
    };
  }

  getScenarios() {
    const scenariosBuilder = new ScenariosBuilder();
    const scenarioFunctions = {
      run_smoke: this.addSmokeScenario.bind(this, scenariosBuilder),
      run_average: this.addAverageScenario.bind(this, scenariosBuilder),
      run_stress: this.addStressScenario.bind(this, scenariosBuilder),
      run_spike: this.addSpikeScenario.bind(this, scenariosBuilder),
    };

    Object.keys(scenarioFunctions).forEach(key => {
      if (this.utils.getCLIVariable(key) !== 'false') {
        scenarioFunctions[key]();
      }
    });

    return scenariosBuilder.build();
  }

  addSmokeScenario(scenariosBuilder) {
    scenariosBuilder.addSmokeScenario(this.smokeConfig, this.nextStartTime);
    this.advanceStartTime(this.smokeConfig.duration);
  }

  addAverageScenario(scenariosBuilder) {
    const { rise, plateau, fall } = this.averageConfig.duration;
    scenariosBuilder.addAverageScenario(this.averageConfig, this.nextStartTime);
    this.advanceStartTime(rise + plateau + fall);
  }

  addStressScenario(scenariosBuilder) {
    const { rise, plateau, fall } = this.stressConfig.duration;
    scenariosBuilder.addStressScenario(this.stressConfig, this.nextStartTime);
    this.advanceStartTime(rise + plateau + fall);
  }

  addSpikeScenario(scenariosBuilder) {
    const { rise, fall } = this.spikeConfig.duration;
    scenariosBuilder.addSpikeScenario(this.spikeConfig, this.nextStartTime);
    this.advanceStartTime(rise + fall);
  }

  advanceStartTime(profileDuration) {
    this.nextStartTime += profileDuration + this.delay;
  }

  getThresholds() {
    const thresholdsBuilder = new ThresholdsBuilder();
    const thresholdConfigs = {
      run_smoke: { name: 'smoke', config: this.smokeConfig },
      run_average: { name: 'average', config: this.averageConfig },
      run_stress: { name: 'stress', config: this.stressConfig },
      run_spike: { name: 'spike', config: this.spikeConfig },
    };

    Object.keys(thresholdConfigs).forEach(key => {
      if (this.utils.getCLIVariable(key) !== 'false') {
        const { name, config } = thresholdConfigs[key];
        thresholdsBuilder.addThreshold(name, config);
      }
    });

    return thresholdsBuilder.build();
  }
}
