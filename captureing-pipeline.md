# Capturing Pipeline

1. [VMs](#vms)
    1. [Collecting a single trace](#collecting-a-single-trace)
    2. [Collecting multiple traces](#collecting-multiple-traces)

This document describes the different parts of the data capturing pipeline and how they interact with each other.

## VMs

VMs are used to run the browser and capture DNS traffic.
VMs are mainly necessary to isolate the DNS server from other applications, since it is not possible to make Google Chrome use a specific DNS server.
Chrome always uses the system provided way for DNS resolution, thus the DNS server has to be system wide.

The order in which steps are executed often matters, therefore this shall include the reasoning for the steps.

### Collecting a single trace

1. First `fstrm_capture` needs to be started to provide the dnstap logging socket in the system.
2. Restarting Unbound is necessary to make it use the new socket directly, otherwise it may take some time until Unbound connnects.
3. Chrome needs to be started next.
    Chromes startup procedure triggers domain lookups to some Google domains.
    Since Google domains are widespread on the internet, we need to make sure to flush these domains from the cache.
    A new user data dir should be created to ensure that the profile is empty and does not contain data from previous runs.
    We already start the configuration of Chrome, like enabeling all the debug tools we will use later.
4. Now all the processes are initializes to an empty running state.
    Next is to flush Unbounds cache and pre-load the TLD list.
    We can speed up the pre-loading step, by loading a cache dump of an already pre-loaded Unbound instance.
5. Now all the initialization is done.
    We need to mark this in the dnstap file somehow, as we need to split the initializatio from the actual data later.
    A query to a non-existing domain like `start.example.` works well.
6. Now we navigate Chrome to the webpage we want to record and wait until the webpage is finished loading or until the wall-clock timer runns out.
7. The end of the experiment phase can be marked analoge to step 5.

### Collecting multiple traces

Now, we can collect a single trace.
Collecting multiple traces requires some coordination between different parts, like

* Where to get the list of jobs to do
* How do we ensure that during the collection step no errors occured
* OR: how do we fix those errors
* Where to put the temporary files and the final results

After each single run we first need to make sure no errors occured.
For this we perfom a series of sanity checks:

* Are both start/end markers present in the data?
* Is there at least one domain lookup between those markers?
    * Is that lookup for the domain we wanted to crawl?
* Is there any domain which should have been prefetched between these markers?

Given all data for a single domain we also need to verify:

* Is the data coherent in itself?
    * Are all the runs similar to each other?
        If only few exceptions, remove those and re-record them again.
        If there are more exceptions then remove the whole dataset and record everything again.

Since it is now possible, that we have to redo an old run, we have to have some kind of dymanic task queue.
The task queue should be persistent, not require external servers, be modifyable in parallel.
