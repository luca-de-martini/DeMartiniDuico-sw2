<template>
<div class="container my-4">
    <div v-if="!validUid">
        Invalid token uid
    </div>
    <div v-if="validUid">
        <div class="my-2" v-if="!validTicket">
            The ticket is not valid for this Store
        </div>
        <div v-if="validTicket">
            <b-card bg-variant="white" title="Details">
                <div>Position in queue: {{position}}</div>
                <div v-for="(value, key) in tokenInfo" :key="key">
                    {{ key }}: {{ value }}
                </div>
            </b-card>
            <b-row class="my-2">
                <b-col cols="6"><b-button class="h-100" variant="success" block @click="logEntry">Log entry</b-button></b-col>
                <b-col cols="6"><b-button class="h-100" variant="danger" block @click="logExit">Log exit</b-button></b-col>
            </b-row>
        </div>
    </div>
    <b-row>
        <b-col cols="6"><b-button class="h-100" to="/staff" block><b-icon-arrow-left/> Back</b-button>
        </b-col>
    </b-row>
           <b-alert
          :show="successfulActionAlert.countdown"
          dismissible
          fade
          class="position-fixed fixed-bottom m-0 rounded-0"
          style="z-index: 2000;"
          variant="success"
          @dismiss-count-down="successfulActionAlert.countdown=$event"
        >
          {{successfulActionAlert.message}}
        </b-alert>
        <b-alert
          :show="failedActionAlert.countdown"
          dismissible
          fade
          class="position-fixed fixed-bottom m-0 rounded-0"
          style="z-index: 2000;"
          variant="danger"
          @dismiss-count-down="failedActionAlert.countdown=$event"
        >
          {{failedActionAlert.message}}
        </b-alert>
</div>
</template>
<script>
export default {
    props:{
    },
    data(){
        return {
            uid: null,
            loadInfoInterval: {},
            tokenInfo: {},
            tickets: {},
            successfulActionAlert:{
                countdown: 0,
                message: "Successful action",
            },
            failedActionAlert:{
                countdown: 0,
                message: "Failed action",
            }
        }
    },
    watch:{
        $route(to){
            this.onRouteChange(to)
        },
    },
    methods:{
        onRouteChange(to){
            this.uid = to.params.uid
            this.loadInfo()
        },
        fetchQueue(shop_id){
            if(!shop_id)
              return
            return this.$api.get(`/staff/shop/${shop_id}/ticket/queue`)
                .then(res => {
                    if(res.status == '200'){
                        this.tickets = res.data;
                        return this.tickets
                    }
                }).catch( console.log )
        },
        loadInfo(){
            if(!this.uid)
                console.log("No :uid provided")

            let whoami = this.$store.dispatch('fetchStaffWhoami')
            whoami.then( data => this.fetchQueue(data.shop_id))
            whoami.then( data => {
                if(!data.shop_id)
                    return
                let shop_id = data.shop_id
                return this.$api.get(`/staff/shop/${shop_id}/token/info?uid=`+encodeURIComponent(this.uid))
                .then( (res) => {
                    this.tokenInfo = res.data
                }).catch(console.log)
            })
        },
        logEntry(){
            this.logAction('log-entry')
        },
        logExit(){
            this.logAction('log-exit')
        },
        logAction(endpoint){
            if(!(endpoint == 'log-entry' || endpoint == 'log-exit'))
                return
            let shop_id = this.$store.state.staff.shop_id
            if(!this.uid || !shop_id){
                alert("Missing data required to perform this action.")
                return
            }
            this.$api.post(`/staff/shop/${shop_id}/token/${endpoint}`,
                { uid: this.uid }
            ).then( (response) => {
                response;
                this.showSuccessfulActionAlert("Successfully executed action: "+endpoint)
            })
            .catch((err) => this.showFailedActionAlert("Operation failed"+(err.response.data?":\n":"")+err.response.data))
        },
        showSuccessfulActionAlert(message){
            this.successfulActionAlert.message = message
            this.successfulActionAlert.countdown = 3
        },
        showFailedActionAlert(message){
            this.failedActionAlert.message = message
            this.failedActionAlert.countdown = 3
        },
    },
    computed:{
        validUid(){
            return (typeof this.uid === 'string') && this.uid.length > 4
        },
        validTicket(){
            return !!this.tokenInfo
        },
        position(){
            if(!(this.tickets instanceof Array)){
              return "Calculating..."
            }
            let index = this.tickets.map(t => t.uid).indexOf(this.uid)
            if(index===-1)
              return "Not present"
            else  
              return index+1
        }
    },
    created(){
        this.onRouteChange(this.$route)
    },
}
</script>