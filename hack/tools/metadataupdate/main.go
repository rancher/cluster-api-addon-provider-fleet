package main

import (
	"flag"
	"fmt"
	"log"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"

	"gopkg.in/yaml.v3"
)

type Metadata struct {
	APIVersion    string          `yaml:"apiVersion"`
	ReleaseSeries []ReleaseSeries `yaml:"releaseSeries"`
}

type ReleaseSeries struct {
	Major    int    `yaml:"major"`
	Minor    int    `yaml:"minor"`
	Contract string `yaml:"contract"`
}

func main() {
	contractFlag := flag.String("contract", "v1beta1", "Contract value for new release entry")
	repoDir := flag.String("repo-dir", ".", "Root directory of the repository")
	flag.Parse()

	tag := os.Getenv("GITHUB_REF_NAME")
	if tag == "" {
		log.Fatal("GITHUB_REF_NAME environment variable not set")
	}

	log.Printf("Processing tag: %s", tag)

	major, minor, isPatch, err := parseTag(tag)
	if err != nil {
		log.Fatal(err)
	}

	if isPatch {
		log.Printf("Skipping patch release: %s", tag)
		return
	}

	log.Printf("Adding new release with major: %d, minor: %d+1, contract: %s", major, minor, *contractFlag)

	metadataPath := filepath.Join(*repoDir, "metadata.yaml")

	metadata, err := readMetadata(metadataPath)
	if err != nil {
		log.Fatalf("Failed to read metadata file: %v", err)
	}

	newRelease := ReleaseSeries{
		Major:    major,
		Minor:    minor + 1,
		Contract: *contractFlag,
	}

	for _, release := range metadata.ReleaseSeries {
		if release.Major == newRelease.Major && release.Minor == newRelease.Minor {
			log.Printf("Release %d.%d already exists, skipping", newRelease.Major, newRelease.Minor)
			return
		}
	}

	metadata.ReleaseSeries = append(metadata.ReleaseSeries, newRelease)

	if err := writeMetadata(metadataPath, metadata); err != nil {
		log.Fatalf("Failed to write metadata file: %v", err)
	}

	log.Printf("Successfully updated metadata.yaml with new release: %d.%d", newRelease.Major, newRelease.Minor)
}

func parseTag(tag string) (int, int, bool, error) {
	// Regular expression to match version tags like v0.8.0 or v0.8.1
	re := regexp.MustCompile(`^v(\d+)\.(\d+)\.(\d+)$`)
	matches := re.FindStringSubmatch(tag)

	if len(matches) != 4 {
		return 0, 0, false, fmt.Errorf("invalid tag format: %s", tag)
	}

	major, err := strconv.Atoi(matches[1])
	if err != nil {
		return 0, 0, false, fmt.Errorf("invalid major version: %s", matches[1])
	}

	minor, err := strconv.Atoi(matches[2])
	if err != nil {
		return 0, 0, false, fmt.Errorf("invalid minor version: %s", matches[2])
	}

	patch, err := strconv.Atoi(matches[3])
	if err != nil {
		return 0, 0, false, fmt.Errorf("invalid patch version: %s", matches[3])
	}

	isPatch := patch > 0

	return major, minor, isPatch, nil
}

func readMetadata(path string) (Metadata, error) {
	var metadata Metadata

	data, err := os.ReadFile(path)
	if err != nil {
		return metadata, err
	}

	err = yaml.Unmarshal(data, &metadata)
	if err != nil {
		return metadata, err
	}

	return metadata, nil
}

func writeMetadata(path string, metadata Metadata) error {
	var sb strings.Builder
	encoder := yaml.NewEncoder(&sb)
	encoder.SetIndent(2)
	defer encoder.Close()

	if err := encoder.Encode(metadata); err != nil {
		return err
	}

	return os.WriteFile(path, []byte(sb.String()), 0644)
}
